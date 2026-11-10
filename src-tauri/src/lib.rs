pub mod credentials;
pub mod models;
pub mod parser;
pub mod paths;
pub mod process_manager;
pub mod quota;
pub mod switcher;
pub mod watcher;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::models::{AccountProfile, ActiveSession, AppSettings, SwitchResult};

#[tauri::command]
async fn cmd_get_active_session() -> Result<ActiveSession, String> {
    switcher::get_active_session().await
}

#[tauri::command]
fn cmd_list_profiles() -> Result<Vec<AccountProfile>, String> {
    Ok(switcher::load_profiles())
}

#[tauri::command]
async fn cmd_save_current_account(label: Option<String>) -> Result<AccountProfile, String> {
    switcher::save_current_account(label).await
}

#[tauri::command]
fn cmd_add_account_tokens(
    label: String,
    email: String,
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
) -> Result<AccountProfile, String> {
    switcher::add_account_tokens(label, email, access_token, refresh_token, id_token)
}

#[tauri::command]
fn cmd_switch_account(profile_id: String) -> Result<SwitchResult, String> {
    switcher::switch_account(&profile_id)
}

#[tauri::command]
fn cmd_delete_profile(profile_id: String) -> Result<(), String> {
    switcher::delete_profile(&profile_id)
}

#[tauri::command]
fn cmd_mark_exhausted(profile_id: String, cooldown_minutes: Option<u32>) -> Result<AccountProfile, String> {
    switcher::mark_exhausted(&profile_id, cooldown_minutes)
}

#[tauri::command]
fn cmd_mark_available(profile_id: String) -> Result<AccountProfile, String> {
    switcher::mark_available(&profile_id)
}

#[tauri::command]
fn cmd_auto_switch_next() -> Result<SwitchResult, String> {
    switcher::auto_switch_next()
}

#[tauri::command]
async fn cmd_refresh_quotas() -> Result<Vec<AccountProfile>, String> {
    switcher::refresh_all_quotas().await
}

#[tauri::command]
fn cmd_get_settings() -> Result<AppSettings, String> {
    Ok(switcher::load_settings())
}

#[tauri::command]
fn cmd_save_settings(settings: AppSettings) -> Result<(), String> {
    switcher::save_settings(&settings)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            let show_item = MenuItem::with_id(&handle, "show", "Show Switcher Dashboard", true, None::<&str>)?;
            let auto_switch_item = MenuItem::with_id(&handle, "auto_switch", "⚡ Switch to Next Account", true, None::<&str>)?;
            let refresh_quota_item = MenuItem::with_id(&handle, "refresh_quotas", "🔄 Refresh Quotas", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(&handle, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(&handle, &[&show_item, &auto_switch_item, &refresh_quota_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Antigravity Account Switcher")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(move |app_h, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app_h.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "auto_switch" => {
                        if let Ok(res) = switcher::auto_switch_next() {
                            println!("[Tray] Switched to {}", res.message);
                        }
                    }
                    "refresh_quotas" => {
                        let _ = tauri::async_runtime::spawn(async {
                            let _ = switcher::refresh_all_quotas().await;
                        });
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app_h = tray.app_handle();
                        if let Some(window) = app_h.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Start background log watcher for rate limits
            watcher::start_log_watcher(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_get_active_session,
            cmd_list_profiles,
            cmd_save_current_account,
            cmd_add_account_tokens,
            cmd_switch_account,
            cmd_delete_profile,
            cmd_mark_exhausted,
            cmd_mark_available,
            cmd_auto_switch_next,
            cmd_refresh_quotas,
            cmd_get_settings,
            cmd_save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Antigravity Account Switcher");
}
