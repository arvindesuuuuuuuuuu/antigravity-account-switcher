use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::paths::find_language_server_log;
use crate::switcher::{auto_switch_next, get_active_session_sync, load_settings, mark_exhausted};

static LAST_TRIGGER_SECS: AtomicI64 = AtomicI64::new(0);

pub fn start_log_watcher(app_handle: AppHandle) {
    thread::spawn(move || {
        let mut last_size: u64 = 0;
        let mut current_path = find_language_server_log();

        // If file exists, start seeking near the end
        if let Some(ref path) = current_path {
            if let Ok(metadata) = std::fs::metadata(path) {
                last_size = metadata.len().saturating_sub(4096);
            }
        }

        loop {
            thread::sleep(Duration::from_secs(2));

            // Refresh path if previously not found
            if current_path.is_none() {
                current_path = find_language_server_log();
                if let Some(ref path) = current_path {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        last_size = metadata.len();
                    }
                }
                continue;
            }

            let path = current_path.as_ref().unwrap();
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => {
                    current_path = None;
                    continue;
                }
            };

            let new_size = metadata.len();
            if new_size <= last_size {
                if new_size < last_size {
                    // File was truncated or rotated
                    last_size = 0;
                }
                continue;
            }

            if let Ok(mut file) = File::open(path) {
                if file.seek(SeekFrom::Start(last_size)).is_ok() {
                    let reader = BufReader::new(file);
                    for line_res in reader.lines() {
                        if let Ok(line) = line_res {
                            check_line_for_rate_limit(&app_handle, &line);
                        }
                    }
                }
            }

            last_size = new_size;
        }
    });
}

fn check_line_for_rate_limit(app: &AppHandle, line: &str) {
    let lower = line.to_ascii_lowercase();
    let is_exhausted = lower.contains("resource_exhausted")
        || lower.contains("quota exceeded")
        || lower.contains("rate limit exceeded")
        || lower.contains("too many requests")
        || (lower.contains("status code 429") || lower.contains("http 429") || lower.contains(" 429 "));

    if !is_exhausted {
        return;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let last = LAST_TRIGGER_SECS.load(Ordering::Relaxed);
    if now - last < 30 {
        // Debounce within 30 seconds
        return;
    }
    LAST_TRIGGER_SECS.store(now, Ordering::Relaxed);

    println!("[Watcher] Detected rate limit in logs: {}", line);

    let settings = load_settings();

    // Mark current active profile as exhausted
    if let Ok(session) = get_active_session_sync() {
        if let Some(profile_id) = session.matched_profile_id {
            let _ = mark_exhausted(&profile_id, Some(settings.default_cooldown_mins));
        }
    }

    // Emit notification event to frontend
    let _ = app.emit("rate-limit-detected", serde_json::json!({
        "timestamp": now,
        "message": "Usage limit reached for the active account."
    }));

    // If auto-switch is enabled, perform auto-switch to next account
    if settings.auto_switch_on_limit {
        match auto_switch_next() {
            Ok(res) => {
                println!("[Watcher] Auto-switched successfully: {}", res.message);
                let _ = app.emit("account-switched", serde_json::json!({
                    "profile": res.profile,
                    "auto": true,
                    "message": res.message
                }));
            }
            Err(err) => {
                println!("[Watcher] Auto-switch failed: {}", err);
                let _ = app.emit("auto-switch-failed", serde_json::json!({
                    "error": err
                }));
            }
        }
    }
}
