use crate::{port, AppState};
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub platform: String,
    pub data_dir: String,
    pub log_dir: String,
    pub proxy_port: u16,
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: "m3vpn".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        data_dir: state.paths.data_dir.display().to_string(),
        log_dir: state.paths.log_dir.display().to_string(),
        proxy_port: state.proxy_port.load(std::sync::atomic::Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    crate::store::all(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    if key.is_empty() || key.len() > 128 {
        return Err("invalid setting key".to_string());
    }
    if value.len() > 8192 {
        return Err("setting value is too large".to_string());
    }
    state
        .set_setting(&key, &value)
        .map_err(|error| error.to_string())?;
    tracing::debug!(key = %key, "setting updated");
    Ok(())
}

#[tauri::command]
pub fn pick_free_port(state: State<'_, AppState>, start: u16) -> Result<u16, String> {
    let start = if start == 0 { 10808 } else { start.max(1024) };
    let port = port::find_free_port(start).map_err(|error| error.to_string())?;
    state
        .proxy_port
        .store(port, std::sync::atomic::Ordering::Relaxed);
    state
        .set_setting("proxy.active_port", &port.to_string())
        .map_err(|error| error.to_string())?;
    tracing::info!(port, "selected free proxy port");
    Ok(port)
}

#[tauri::command]
pub fn open_logs_dir(state: State<'_, AppState>) -> Result<(), String> {
    open::that(&state.paths.log_dir).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    let message = message.chars().take(2000).collect::<String>();
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        "debug" => tracing::debug!(target: "frontend", "{message}"),
        _ => tracing::info!(target: "frontend", "{message}"),
    }
}
