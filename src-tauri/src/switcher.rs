use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::credentials::{read_app_storage_email, read_current_credentials, write_credentials};
use crate::models::{AccountProfile, ActiveSession, AppSettings, SwitchResult};
use crate::parser::parse_user_status;
use crate::paths::{
    find_antigravity_db, find_app_storage_file, get_jetski_token_file,
    get_profiles_file_path, get_settings_file_path,
};
use crate::process_manager::{get_language_server_endpoint, kill_language_server_subprocesses};
use crate::quota::{
    ensure_valid_access_token, fetch_local_language_server_quota, fetch_upstream_quota,
};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn load_settings() -> AppSettings {
    let path = get_settings_file_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str::<AppSettings>(&data) {
            return settings;
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_file_path();
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

pub fn load_profiles() -> Vec<AccountProfile> {
    let path = get_profiles_file_path();
    let mut profiles = if let Ok(data) = fs::read_to_string(&path) {
        serde_json::from_str::<Vec<AccountProfile>>(&data).unwrap_or_default()
    } else {
        Vec::new()
    };

    let current_time = now_secs();
    let mut modified = false;
    for p in &mut profiles {
        if p.is_exhausted {
            if let Some(until) = p.exhausted_until {
                if until <= current_time {
                    p.is_exhausted = false;
                    p.exhausted_until = None;
                    modified = true;
                }
            }
        }
    }

    if modified {
        let _ = save_profiles_internal(&profiles);
    }

    profiles
}

fn save_profiles_internal(profiles: &[AccountProfile]) -> Result<(), String> {
    let path = get_profiles_file_path();
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write profiles: {}", e))?;
    Ok(())
}

/// Reads the active session synchronously from local storage & config files
pub fn get_active_session_sync() -> Result<ActiveSession, String> {
    let mut email = String::new();
    let mut name = String::new();
    let mut tier = "Standard".to_string();
    let mut picture: Option<String> = None;
    let mut has_access_token = false;
    let mut has_refresh_token = false;

    // 1. Try reading from Standalone Token file
    if let Some(cred) = read_current_credentials() {
        if let Some(ref em) = cred.id_token_email {
            email = em.clone();
        }
        has_access_token = !cred.token.access_token.is_empty();
        has_refresh_token = !cred.token.refresh_token.is_empty();
    }

    // 2. Try reading from Antigravity's app_storage.json
    if email.is_empty() {
        if let Some(storage_email) = read_app_storage_email() {
            email = storage_email;
        }
    }

    // 3. Try reading from Antigravity's SQLite DB (state.vscdb) if it exists
    let db_path = find_antigravity_db();
    if let Some(ref db) = db_path {
        if let Ok(conn) = Connection::open(db) {
            conn.execute_batch("PRAGMA busy_timeout = 3000;").ok();
            if let Ok(mut stmt) = conn.prepare("SELECT value FROM ItemTable WHERE key = ?1") {
                if let Ok(status_raw) = stmt.query_row(params!["antigravityUnifiedStateSync.userStatus"], |r| r.get::<_, String>(0)) {
                    if let Ok(identity) = parse_user_status(&status_raw) {
                        if !identity.email.is_empty() {
                            email = identity.email;
                        }
                        if !identity.name.is_empty() {
                            name = identity.name;
                        }
                        if !identity.tier.is_empty() {
                            tier = identity.tier;
                        }
                        picture = identity.picture;
                    }
                }
            }
        }
    }

    if email.is_empty() {
        email = "antigravity.user@gmail.com".to_string();
    }
    if name.is_empty() {
        name = email.split('@').next().unwrap_or("Antigravity User").to_string();
    }

    let profiles = load_profiles();
    let matched = profiles.iter().find(|p| p.email.eq_ignore_ascii_case(&email));

    let app_storage_path = find_app_storage_file().map(|p| p.to_string_lossy().to_string());
    let jetski_token_path = {
        let p = get_jetski_token_file();
        if p.exists() {
            Some(p.to_string_lossy().to_string())
        } else {
            None
        }
    };

    Ok(ActiveSession {
        email: email.clone(),
        name,
        tier: matched.map(|p| p.tier.clone()).unwrap_or(tier),
        picture: picture.or_else(|| matched.and_then(|p| p.picture.clone())),
        has_access_token: has_access_token || matched.map(|p| p.access_token.is_some()).unwrap_or(false),
        has_refresh_token: has_refresh_token || matched.map(|p| p.refresh_token.is_some()).unwrap_or(false),
        app_storage_path,
        jetski_token_path,
        db_path: db_path.map(|p| p.to_string_lossy().to_string()),
        is_saved: matched.is_some(),
        matched_profile_id: matched.map(|p| p.id.clone()),
        rolling_5h_percentage: matched.and_then(|p| p.rolling_5h_percentage),
        rolling_5h_reset: matched.and_then(|p| p.rolling_5h_reset.clone()),
        weekly_percentage: matched.and_then(|p| p.weekly_percentage),
        weekly_reset: matched.and_then(|p| p.weekly_reset.clone()),
        model_groups: matched.map(|p| p.model_groups.clone()).unwrap_or_default(),
        prompt_credits: matched.and_then(|p| p.prompt_credits),
        flow_credits: matched.and_then(|p| p.flow_credits),
    })
}

/// Reads active session and enriches with live RPC query from running Antigravity Language Server
pub async fn get_active_session() -> Result<ActiveSession, String> {
    let mut session = get_active_session_sync()?;

    // If language_server is running, query live local RPC endpoint
    if let Some(endpoint) = get_language_server_endpoint() {
        if let Ok(live_quota) = fetch_local_language_server_quota(&endpoint).await {
            if !live_quota.email.is_empty() {
                session.email = live_quota.email.clone();
                if session.name == "Antigravity User" || session.name.is_empty() {
                    session.name = live_quota.email.split('@').next().unwrap_or("Antigravity User").to_string();
                }
            }
            if let Some(ref name) = live_quota.name {
                if !name.is_empty() {
                    session.name = name.clone();
                }
            }
            if let Some(ref tier) = live_quota.tier {
                if !tier.is_empty() {
                    session.tier = tier.clone();
                }
            }
            if let Some(p5h) = live_quota.rolling_5h_percentage {
                session.rolling_5h_percentage = Some(p5h);
                session.rolling_5h_reset = live_quota.rolling_5h_reset_time;
            }
            if let Some(pwk) = live_quota.weekly_percentage {
                session.weekly_percentage = Some(pwk);
                session.weekly_reset = live_quota.weekly_reset_time;
            }
            if !live_quota.model_groups.is_empty() {
                session.model_groups = live_quota.model_groups;
            }
            if live_quota.prompt_credits.is_some() {
                session.prompt_credits = live_quota.prompt_credits;
            }
            if live_quota.flow_credits.is_some() {
                session.flow_credits = live_quota.flow_credits;
            }

            // Re-match against saved profiles with updated email
            let profiles = load_profiles();
            if let Some(matched) = profiles.iter().find(|p| p.email.eq_ignore_ascii_case(&session.email)) {
                session.is_saved = true;
                session.matched_profile_id = Some(matched.id.clone());
                if session.picture.is_none() {
                    session.picture = matched.picture.clone();
                }
            }
        }
    }

    Ok(session)
}

pub async fn save_current_account(custom_label: Option<String>) -> Result<AccountProfile, String> {
    let session = get_active_session().await?;
    let creds_opt = read_current_credentials();

    let access_token = creds_opt.as_ref().map(|c| c.token.access_token.clone());
    let refresh_token = creds_opt.as_ref().map(|c| c.token.refresh_token.clone());
    let id_token = creds_opt.as_ref().and_then(|c| c.id_token.clone());

    let mut oauth_token_str = None;
    let mut user_status_str = None;
    let mut model_credits_str = None;
    let mut model_preferences_str = None;

    if let Some(db_path) = find_antigravity_db() {
        if let Ok(conn) = Connection::open(&db_path) {
            conn.execute_batch("PRAGMA busy_timeout = 3000;").ok();
            if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM ItemTable WHERE key LIKE 'antigravityUnifiedStateSync.%'") {
                if let Ok(rows) = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))) {
                    for r in rows.flatten() {
                        match r.0.as_str() {
                            "antigravityUnifiedStateSync.oauthToken" => oauth_token_str = Some(r.1),
                            "antigravityUnifiedStateSync.userStatus" => user_status_str = Some(r.1),
                            "antigravityUnifiedStateSync.modelCredits" => model_credits_str = Some(r.1),
                            "antigravityUnifiedStateSync.modelPreferences" => model_preferences_str = Some(r.1),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    let mut profiles = load_profiles();
    let now = now_secs();

    let label = custom_label.unwrap_or_else(|| {
        if session.name != "Antigravity User" {
            format!("{} ({})", session.name, session.tier)
        } else {
            format!("Account {}", profiles.len() + 1)
        }
    });

    for p in &mut profiles {
        p.is_active = false;
    }

    if let Some(pos) = profiles.iter().position(|p| p.email.eq_ignore_ascii_case(&session.email)) {
        let p = &mut profiles[pos];
        p.label = label;
        p.name = session.name;
        p.tier = session.tier;
        p.picture = session.picture;
        p.is_active = true;
        p.last_used = now;
        p.rolling_5h_percentage = session.rolling_5h_percentage;
        p.rolling_5h_reset = session.rolling_5h_reset;
        p.weekly_percentage = session.weekly_percentage;
        p.weekly_reset = session.weekly_reset;
        p.model_groups = session.model_groups;
        p.prompt_credits = session.prompt_credits;
        p.flow_credits = session.flow_credits;
        p.quota_last_updated = Some(now);

        if access_token.is_some() {
            p.access_token = access_token;
        }
        if refresh_token.is_some() {
            p.refresh_token = refresh_token;
        }
        if id_token.is_some() {
            p.id_token = id_token;
        }
        if oauth_token_str.is_some() {
            p.oauth_token = oauth_token_str;
        }
        if user_status_str.is_some() {
            p.user_status = user_status_str;
        }
        if model_credits_str.is_some() {
            p.model_credits = model_credits_str;
        }
        if model_preferences_str.is_some() {
            p.model_preferences = model_preferences_str;
        }
        let saved = p.clone();
        save_profiles_internal(&profiles)?;
        return Ok(saved);
    }

    let new_profile = AccountProfile {
        id: Uuid::new_v4().to_string(),
        label,
        email: session.email,
        name: session.name,
        tier: session.tier,
        picture: session.picture,
        access_token,
        refresh_token,
        expiry_timestamp: Some(now + 3600),
        id_token,
        oauth_token: oauth_token_str,
        user_status: user_status_str,
        model_credits: model_credits_str,
        model_preferences: model_preferences_str,
        is_active: true,
        is_exhausted: false,
        exhausted_until: None,
        last_used: now,
        created_at: now,
        rolling_5h_percentage: session.rolling_5h_percentage,
        rolling_5h_reset: session.rolling_5h_reset,
        weekly_percentage: session.weekly_percentage,
        weekly_reset: session.weekly_reset,
        quota_buckets: Vec::new(),
        model_groups: session.model_groups,
        prompt_credits: session.prompt_credits,
        flow_credits: session.flow_credits,
        quota_last_updated: Some(now),
    };

    profiles.push(new_profile.clone());
    save_profiles_internal(&profiles)?;
    Ok(new_profile)
}

pub fn add_account_tokens(
    label: String,
    email: String,
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
) -> Result<AccountProfile, String> {
    let mut profiles = load_profiles();
    let now = now_secs();

    let new_profile = AccountProfile {
        id: Uuid::new_v4().to_string(),
        label,
        email: email.clone(),
        name: email.split('@').next().unwrap_or("Antigravity User").to_string(),
        tier: "Standard".to_string(),
        picture: None,
        access_token: Some(access_token),
        refresh_token: Some(refresh_token),
        expiry_timestamp: Some(now + 3600),
        id_token,
        oauth_token: None,
        user_status: None,
        model_credits: None,
        model_preferences: None,
        is_active: false,
        is_exhausted: false,
        exhausted_until: None,
        last_used: 0,
        created_at: now,
        rolling_5h_percentage: None,
        rolling_5h_reset: None,
        weekly_percentage: None,
        weekly_reset: None,
        quota_buckets: Vec::new(),
        model_groups: Vec::new(),
        prompt_credits: None,
        flow_credits: None,
        quota_last_updated: None,
    };

    profiles.push(new_profile.clone());
    save_profiles_internal(&profiles)?;
    Ok(new_profile)
}

pub fn switch_account(profile_id: &str) -> Result<SwitchResult, String> {
    let mut profiles = load_profiles();
    let target_idx = profiles
        .iter()
        .position(|p| p.id == profile_id || p.email.eq_ignore_ascii_case(profile_id))
        .ok_or_else(|| format!("Profile '{}' not found", profile_id))?;

    let target = profiles[target_idx].clone();

    // 1. Pre-write credentials to Keyring and fallback files
    write_credentials(&target)?;

    // 2. If Antigravity state.vscdb exists, update SQLite
    if let Some(db_path) = find_antigravity_db() {
        if db_path.exists() {
            let backup_path = db_path.with_extension(format!("vscdb.bak.{}", now_secs()));
            let _ = fs::copy(&db_path, &backup_path);

            if let Ok(conn) = Connection::open(&db_path) {
                conn.execute_batch("PRAGMA busy_timeout = 5000;").ok();
                let upsert = "INSERT INTO ItemTable (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value;";
                if let Some(ref tok) = target.oauth_token {
                    let _ = conn.execute(upsert, params!["antigravityUnifiedStateSync.oauthToken", tok]);
                }
                if let Some(ref st) = target.user_status {
                    let _ = conn.execute(upsert, params!["antigravityUnifiedStateSync.userStatus", st]);
                }
                if let Some(ref mc) = target.model_credits {
                    let _ = conn.execute(upsert, params!["antigravityUnifiedStateSync.modelCredits", mc]);
                }
                if let Some(ref mp) = target.model_preferences {
                    let _ = conn.execute(upsert, params!["antigravityUnifiedStateSync.modelPreferences", mp]);
                }
            }
        }
    }

    // 3. Hot-switch: terminate running language_server child processes
    let killed = kill_language_server_subprocesses().unwrap_or(0);
    let hot_switched = killed > 0;

    // 4. Re-assert credentials to guarantee no race condition from old process
    let _ = write_credentials(&target);

    // 5. Update profiles active state
    let now = now_secs();
    for (idx, p) in profiles.iter_mut().enumerate() {
        if idx == target_idx {
            p.is_active = true;
            p.last_used = now;
        } else {
            p.is_active = false;
        }
    }
    save_profiles_internal(&profiles)?;

    let msg = if hot_switched {
        format!("Hot-switched to {} (reloaded {} language_server process)", target.email, killed)
    } else {
        format!("Switched credentials to {}. Launch or restart Antigravity to take effect.", target.email)
    };

    Ok(SwitchResult {
        success: true,
        message: msg,
        profile: Some(target),
        hot_switched,
    })
}

pub fn auto_switch_next() -> Result<SwitchResult, String> {
    let profiles = load_profiles();
    let now = now_secs();

    let mut available: Vec<_> = profiles
        .iter()
        .filter(|p| !p.is_active && (!p.is_exhausted || p.exhausted_until.unwrap_or(0) <= now))
        .collect();

    available.sort_by_key(|p| p.last_used);

    if let Some(next) = available.first() {
        return switch_account(&next.id);
    }

    let mut exhausted: Vec<_> = profiles
        .iter()
        .filter(|p| !p.is_active && p.is_exhausted)
        .collect();

    exhausted.sort_by_key(|p| p.exhausted_until.unwrap_or(0));

    if let Some(soonest) = exhausted.first() {
        let diff_mins = (soonest.exhausted_until.unwrap_or(0) - now).max(1) / 60;
        return Err(format!(
            "All alternative Google accounts are currently rate-limited. Next account ({}) resets in ~{} mins.",
            soonest.email, diff_mins
        ));
    }

    Err("No other Google account profiles saved. Add one by signing in with your other account in Antigravity or pasting tokens.".to_string())
}

pub fn mark_exhausted(profile_id: &str, cooldown_minutes: Option<u32>) -> Result<AccountProfile, String> {
    let mut profiles = load_profiles();
    let target = profiles
        .iter_mut()
        .find(|p| p.id == profile_id || p.email.eq_ignore_ascii_case(profile_id))
        .ok_or_else(|| format!("Profile '{}' not found", profile_id))?;

    let mins = cooldown_minutes.unwrap_or(300); // 5 hours default
    let now = now_secs();
    target.is_exhausted = true;
    target.exhausted_until = Some(now + (mins as i64 * 60));

    let updated = target.clone();
    save_profiles_internal(&profiles)?;
    Ok(updated)
}

pub fn mark_available(profile_id: &str) -> Result<AccountProfile, String> {
    let mut profiles = load_profiles();
    let target = profiles
        .iter_mut()
        .find(|p| p.id == profile_id || p.email.eq_ignore_ascii_case(profile_id))
        .ok_or_else(|| format!("Profile '{}' not found", profile_id))?;

    target.is_exhausted = false;
    target.exhausted_until = None;

    let updated = target.clone();
    save_profiles_internal(&profiles)?;
    Ok(updated)
}

pub fn delete_profile(profile_id: &str) -> Result<(), String> {
    let mut profiles = load_profiles();
    let initial_len = profiles.len();
    profiles.retain(|p| p.id != profile_id && !p.email.eq_ignore_ascii_case(profile_id));
    if profiles.len() == initial_len {
        return Err(format!("Profile '{}' not found", profile_id));
    }
    save_profiles_internal(&profiles)
}

/// Refreshes quotas for all profiles: queries local language server if active, or upstream API
pub async fn refresh_all_quotas() -> Result<Vec<AccountProfile>, String> {
    let mut profiles = load_profiles();
    let mut modified = false;

    // Check if local language server is active
    let ls_endpoint = get_language_server_endpoint();

    for p in &mut profiles {
        // If this profile matches the running Antigravity instance, use local language server RPC
        if p.is_active {
            if let Some(ref endpoint) = ls_endpoint {
                if let Ok(local_summary) = fetch_local_language_server_quota(endpoint).await {
                    p.rolling_5h_percentage = local_summary.rolling_5h_percentage;
                    p.rolling_5h_reset = local_summary.rolling_5h_reset_time;
                    p.weekly_percentage = local_summary.weekly_percentage;
                    p.weekly_reset = local_summary.weekly_reset_time;
                    p.quota_buckets = local_summary.buckets;
                    p.model_groups = local_summary.model_groups;
                    p.prompt_credits = local_summary.prompt_credits;
                    p.flow_credits = local_summary.flow_credits;
                    p.quota_last_updated = Some(local_summary.last_checked);
                    modified = true;
                    continue;
                }
            }
        }

        // Otherwise, fetch from upstream Google Cloud Code PA API
        let (access_tok, new_refresh, new_exp) = match ensure_valid_access_token(
            p.access_token.as_deref(),
            p.refresh_token.as_deref(),
            p.expiry_timestamp,
        ).await {
            Ok(res) => res,
            Err(e) => {
                eprintln!("[Quota] Token refresh error for {}: {}", p.email, e);
                continue;
            }
        };

        p.access_token = Some(access_tok.clone());
        if new_refresh.is_some() {
            p.refresh_token = new_refresh;
        }
        p.expiry_timestamp = Some(new_exp);

        match fetch_upstream_quota(&access_tok, &p.email).await {
            Ok(summary) => {
                p.rolling_5h_percentage = summary.rolling_5h_percentage;
                p.rolling_5h_reset = summary.rolling_5h_reset_time;
                p.weekly_percentage = summary.weekly_percentage;
                p.weekly_reset = summary.weekly_reset_time;
                p.quota_buckets = summary.buckets;
                p.model_groups = summary.model_groups;
                p.quota_last_updated = Some(summary.last_checked);

                // Auto-detect rate limit if 5h remaining is 0%
                if let Some(pct) = summary.rolling_5h_percentage {
                    if pct <= 0 {
                        p.is_exhausted = true;
                        p.exhausted_until = Some(now_secs() + 18000); // 5 hours
                    }
                }
                modified = true;
            }
            Err(e) => {
                eprintln!("[Quota] Upstream fetch error for {}: {}", p.email, e);
            }
        }
    }

    if modified {
        let _ = save_profiles_internal(&profiles);
    }

    Ok(profiles)
}
