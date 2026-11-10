use std::fs;
use std::process::Command;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::AccountProfile;
use crate::paths::{
    find_app_storage_file, find_credential_user_scope_file, get_antigravity_cli_token_file,
    get_gemini_dir, get_google_accounts_file, get_jetski_token_file, get_oauth_creds_file,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct JetskiTokenDetails {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expiry: String, // RFC3339
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JetskiOAuthTokenFile {
    pub token: JetskiTokenDetails,
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthCredsFile {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expiry_date: i64, // ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleAccountsFile {
    pub active: String,
    pub old: Vec<String>,
}

/// Reads the currently active credentials from the system
pub fn read_current_credentials() -> Option<JetskiOAuthTokenFile> {
    // 1. Check ~/.gemini/jetski-standalone-oauth-token
    let jetski = get_jetski_token_file();
    if jetski.exists() {
        if let Ok(content) = fs::read_to_string(&jetski) {
            if let Ok(token_obj) = serde_json::from_str::<JetskiOAuthTokenFile>(&content) {
                return Some(token_obj);
            }
        }
    }

    // 2. Check ~/.gemini/antigravity-cli/antigravity-oauth-token
    let cli_token = get_antigravity_cli_token_file();
    if cli_token.exists() {
        if let Ok(content) = fs::read_to_string(&cli_token) {
            if let Ok(token_obj) = serde_json::from_str::<JetskiOAuthTokenFile>(&content) {
                return Some(token_obj);
            }
        }
    }

    // 3. Check ~/.gemini/oauth_creds.json
    let oauth_creds = get_oauth_creds_file();
    if oauth_creds.exists() {
        if let Ok(content) = fs::read_to_string(&oauth_creds) {
            if let Ok(creds) = serde_json::from_str::<OAuthCredsFile>(&content) {
                let expiry_sec = creds.expiry_date / 1000;
                let dt = DateTime::from_timestamp(expiry_sec, 0).unwrap_or_else(Utc::now);
                return Some(JetskiOAuthTokenFile {
                    token: JetskiTokenDetails {
                        access_token: creds.access_token,
                        token_type: creds.token_type,
                        refresh_token: creds.refresh_token,
                        expiry: dt.to_rfc3339(),
                        expires_in: Some(3600),
                    },
                    auth_method: "consumer".to_string(),
                    id_token_email: None,
                    id_token: creds.id_token,
                });
            }
        }
    }

    None
}

/// Reads the last logged-in email from Antigravity's app_storage.json
pub fn read_app_storage_email() -> Option<String> {
    let path = find_app_storage_file()?;
    let content = fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data.get("agy.onboarding.lastLoginUsername")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Writes credentials to all Antigravity desktop credential locations (Keyring + Files + Storage)
pub fn write_credentials(profile: &AccountProfile) -> Result<(), String> {
    let gemini_dir = get_gemini_dir();
    let _ = fs::create_dir_all(&gemini_dir);

    let access_token = profile.access_token.clone().unwrap_or_default();
    let refresh_token = profile.refresh_token.clone().unwrap_or_default();
    let expiry_timestamp = profile.expiry_timestamp.unwrap_or_else(|| {
        chrono::Utc::now().timestamp() + 3600
    });

    let expiry_secs = if expiry_timestamp > 10_000_000_000 {
        expiry_timestamp / 1000
    } else {
        expiry_timestamp
    };
    let expiry_dt = DateTime::from_timestamp(expiry_secs, 0).unwrap_or_else(Utc::now);
    let expiry_str = expiry_dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

    let jetski_payload = JetskiOAuthTokenFile {
        token: JetskiTokenDetails {
            access_token: access_token.clone(),
            token_type: "Bearer".to_string(),
            refresh_token: refresh_token.clone(),
            expiry: expiry_str,
            expires_in: Some(3599),
        },
        auth_method: "consumer".to_string(),
        id_token_email: Some(profile.email.clone()),
        id_token: profile.id_token.clone(),
    };

    let jetski_json = serde_json::to_string_pretty(&jetski_payload)
        .map_err(|e| format!("Failed to serialize token file: {}", e))?;

    // 1. Write ~/.gemini/jetski-standalone-oauth-token
    let standalone_path = get_jetski_token_file();
    if let Err(e) = fs::write(&standalone_path, &jetski_json) {
        eprintln!("[Credentials] Warning: failed to write jetski-standalone-oauth-token: {}", e);
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&standalone_path, fs::Permissions::from_mode(0o600));
        }
    }

    // 2. Write ~/.gemini/antigravity-cli/antigravity-oauth-token
    let cli_path = get_antigravity_cli_token_file();
    if let Some(parent) = cli_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cli_path, &jetski_json);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&cli_path, fs::Permissions::from_mode(0o600));
    }

    // 3. Write ~/.gemini/oauth_creds.json
    let oauth_creds_path = get_oauth_creds_file();
    let oauth_creds = OAuthCredsFile {
        access_token: access_token.clone(),
        refresh_token: refresh_token.clone(),
        token_type: "Bearer".to_string(),
        expiry_date: expiry_secs * 1000,
        id_token: profile.id_token.clone(),
        scope: "https://www.googleapis.com/auth/userinfo.email openid https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.profile".to_string(),
    };
    if let Ok(oauth_json) = serde_json::to_string_pretty(&oauth_creds) {
        let _ = fs::write(&oauth_creds_path, oauth_json);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&oauth_creds_path, fs::Permissions::from_mode(0o600));
        }
    }

    // 4. Write ~/.gemini/google_accounts.json
    let google_accounts_path = get_google_accounts_file();
    let accounts_data = GoogleAccountsFile {
        active: profile.email.clone(),
        old: vec![],
    };
    if let Ok(acc_json) = serde_json::to_string_pretty(&accounts_data) {
        let _ = fs::write(&google_accounts_path, acc_json);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&google_accounts_path, fs::Permissions::from_mode(0o600));
        }
    }

    // 5. Update Antigravity's app_storage.json
    if let Some(storage_path) = find_app_storage_file() {
        if let Ok(content) = fs::read_to_string(&storage_path) {
            if let Ok(mut json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(obj) = json_val.as_object_mut() {
                    obj.insert(
                        "agy.onboarding.lastLoginUsername".to_string(),
                        serde_json::Value::String(profile.email.clone()),
                    );
                    if let Ok(updated_str) = serde_json::to_string_pretty(&json_val) {
                        let _ = fs::write(&storage_path, updated_str);
                    }
                }
            }
        }
    }

    // 6. Update ~/.config/antigravity/credential-user-scope if exists
    if let Some(scope_path) = find_credential_user_scope_file() {
        // preserve or touch
        let _ = fs::write(&scope_path, "u40fd6df0c35681e2\n");
    }

    // 7. Write to System Keyring (Windows Credential Manager / Linux Secret Service)
    write_to_system_keyring(&jetski_json);

    Ok(())
}

fn write_to_system_keyring(payload_json: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        #[repr(C)]
        struct FILETIME {
            dw_low_date_time: u32,
            dw_high_date_time: u32,
        }

        #[repr(C)]
        struct CREDENTIALW {
            flags: u32,
            cred_type: u32,
            target_name: *const u16,
            comment: *const u16,
            last_written: FILETIME,
            credential_blob_size: u32,
            credential_blob: *const u8,
            persist: u32,
            attribute_count: u32,
            attributes: *const std::ffi::c_void,
            target_alias: *const u16,
            user_name: *const u16,
        }

        #[link(name = "advapi32")]
        extern "system" {
            fn CredWriteW(credential: *const CREDENTIALW, flags: u32) -> i32;
            fn CredDeleteW(target_name: *const u16, type_: u32, flags: u32) -> i32;
        }

        let target = "gemini:antigravity";
        let user = "antigravity";
        let secret = payload_json.as_bytes();

        let target_wide: Vec<u16> = std::ffi::OsStr::new(target)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let user_wide: Vec<u16> = std::ffi::OsStr::new(user)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let cred = CREDENTIALW {
            flags: 0,
            cred_type: 1, // CRED_TYPE_GENERIC
            target_name: target_wide.as_ptr(),
            comment: ptr::null(),
            last_written: FILETIME {
                dw_low_date_time: 0,
                dw_high_date_time: 0,
            },
            credential_blob_size: secret.len() as u32,
            credential_blob: secret.as_ptr(),
            persist: 2, // CRED_PERSIST_LOCAL_MACHINE
            attribute_count: 0,
            attributes: ptr::null(),
            target_alias: ptr::null(),
            user_name: user_wide.as_ptr(),
        };

        unsafe {
            let _ = CredDeleteW(target_wide.as_ptr(), 1, 0);
            let _ = CredWriteW(&cred, 0);
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::io::Write;

        // Try writing to secret-tool if available
        let mut child = match Command::new("secret-tool")
            .args(["store", "--label=gemini", "service", "gemini", "username", "antigravity"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return, // secret-tool not installed, file credentials suffice
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload_json.as_bytes());
        }
        let _ = child.wait();
    }
}
