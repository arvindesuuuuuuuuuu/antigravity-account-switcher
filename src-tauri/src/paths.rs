use std::path::PathBuf;

/// Gets the local configuration directory for this switcher app
pub fn get_app_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("antigravity-account-switcher");
        }
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".config").join("antigravity-account-switcher");
    }

    PathBuf::from(".antigravity-account-switcher")
}

pub fn get_profiles_file_path() -> PathBuf {
    let dir = get_app_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("profiles.json")
}

pub fn get_settings_file_path() -> PathBuf {
    let dir = get_app_config_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("settings.json")
}

/// Resolves the Antigravity (classic/standalone Electron app) storage directory.
/// STRICT REQUIREMENT: Antigravity ONLY. Strictly NO "Antigravity IDE".
pub fn get_antigravity_storage_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("Antigravity");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".config").join("Antigravity");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Discovers the Antigravity desktop app_storage.json
pub fn find_app_storage_file() -> Option<PathBuf> {
    if let Some(dir) = get_antigravity_storage_dir() {
        let p = dir.join("app_storage.json");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Discovers ~/.config/antigravity/credential-user-scope (Linux) or Windows equivalent
pub fn find_credential_user_scope_file() -> Option<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join("antigravity").join("credential-user-scope");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Resolves ~/.gemini directory where Antigravity standalone & language_server store fallback tokens
pub fn get_gemini_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".gemini"))
        .unwrap_or_else(|| PathBuf::from(".gemini"))
}

/// Resolves ~/.gemini/jetski-standalone-oauth-token
pub fn get_jetski_token_file() -> PathBuf {
    get_gemini_dir().join("jetski-standalone-oauth-token")
}

/// Resolves ~/.gemini/antigravity-cli/antigravity-oauth-token
pub fn get_antigravity_cli_token_file() -> PathBuf {
    get_gemini_dir().join("antigravity-cli").join("antigravity-oauth-token")
}

/// Resolves ~/.gemini/oauth_creds.json
pub fn get_oauth_creds_file() -> PathBuf {
    get_gemini_dir().join("oauth_creds.json")
}

/// Resolves ~/.gemini/google_accounts.json
pub fn get_google_accounts_file() -> PathBuf {
    get_gemini_dir().join("google_accounts.json")
}

/// Discovers state.vscdb SQLite file STRICTLY under Antigravity (classic desktop app).
/// STRICT REQUIREMENT: strictly NO "Antigravity IDE".
pub fn find_antigravity_db() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(&appdata)
                .join("Antigravity")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = dirs::home_dir() {
            let p = home
                .join(".config")
                .join("Antigravity")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Discovers the Antigravity language_server.log file for rate limit monitoring
pub fn find_language_server_log() -> Option<PathBuf> {
    if let Some(dir) = get_antigravity_storage_dir() {
        let p = dir.join("logs").join("language_server.log");
        if p.exists() {
            return Some(p);
        }
    }

    // Also check ~/.gemini/antigravity/cli.log or log directory as fallback
    let gemini_log = get_gemini_dir().join("antigravity").join("cli.log");
    if gemini_log.exists() {
        return Some(gemini_log);
    }

    None
}
