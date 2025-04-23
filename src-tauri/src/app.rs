use log::warn;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::config::Settings;

#[tauri::command]
fn set_config(app_handle: AppHandle, config: Settings) -> Result<(), String> {
    let old_config = Settings::load_or_create(true).map_err(|e| e.to_string())?;

    set_autostart(&app_handle, old_config.autostart, config.autostart)
        .map_err(|e| e.to_string())?;

    config.store().map_err(|e| e.to_string())?;
    Ok(())
}

fn set_autostart<R: Runtime, T: Manager<R>>(app: &T, old: bool, new: bool) -> anyhow::Result<()> {
    match (old, new) {
        (false, true) => app.autolaunch().enable()?,
        (true, false) => app.autolaunch().disable()?,
        _ => {}
    }
    Ok(())
}

#[tauri::command]
fn get_config() -> Result<Settings, String> {
    Settings::load_or_create(true).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            match app.get_webview_window("main") {
                Some(w) => _ = w.set_focus(),
                None => warn!("no frontend window found"),
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["-s"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_autostart = match Settings::load_or_create(true) {
                Ok(c) => c.autostart,
                Err(_) => false,
            };
            let current_autostart = app.autolaunch().is_enabled().unwrap_or(false);
            set_autostart(app, current_autostart, config_autostart)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![set_config, get_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
