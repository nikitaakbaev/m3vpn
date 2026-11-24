use crate::core::{self, ConnectionStatus};
use crate::port;
use crate::store::{ProfileRow, ServerRow};
use crate::subscription::{self, ImportSummary};
use crate::AppState;
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInfo {
    pub available: bool,
    pub xray_path: Option<String>,
    pub xray_version: Option<String>,
    pub assets_dir: Option<String>,
    pub error: Option<String>,
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
    {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "core lock poisoned".to_string())?;
        if core.process.is_some() {
            return Err("Сначала отключите соединение, затем меняйте порт".to_string());
        }
        core.socks_port = port;
        core.http_port = port.saturating_add(1);
    }
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
pub fn open_core_dir(state: State<'_, AppState>) -> Result<(), String> {
    open::that(&state.paths.core_dir).map_err(|error| error.to_string())
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

#[tauri::command]
pub fn import_subscription(
    state: State<'_, AppState>,
    url: String,
    name: Option<String>,
) -> Result<ImportSummary, String> {
    subscription::import(&state.db, &url, name).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileRow>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    crate::store::list_profiles(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_servers(
    state: State<'_, AppState>,
    profile_id: Option<String>,
) -> Result<Vec<ServerRow>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    crate::store::list_servers(&db, profile_id.as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, profile_id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    crate::store::delete_profile(&db, &profile_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn connect_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<ConnectionStatus, String> {
    let sidecars = state
        .sidecars
        .as_ref()
        .ok_or_else(|| "ядра не найдены: запусти scripts/build-sidecars.sh".to_string())?;

    let (name, config_text) = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        crate::store::server_config(&db, &server_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "сервер не найден".to_string())?
    };

    let config: serde_json::Value =
        serde_json::from_str(&config_text).map_err(|error| format!("битый конфиг: {error}"))?;

    let status = core::connect(
        &state.core,
        &state.paths,
        sidecars,
        &server_id,
        &name,
        config,
    )
    .map_err(|error| format!("{error:#}"))?;

    let _ = state.set_setting("last.server_id", &server_id);
    Ok(status)
}

#[tauri::command]
pub fn disconnect_core(state: State<'_, AppState>) -> ConnectionStatus {
    core::disconnect(&state.core, &state.paths)
}

#[tauri::command]
pub fn connection_status(state: State<'_, AppState>) -> ConnectionStatus {
    core::status(&state.core, &state.paths.xray_log)
}

#[tauri::command]
pub fn core_info(state: State<'_, AppState>) -> CoreInfo {
    let Some(sidecars) = state.sidecars.as_ref() else {
        return CoreInfo {
            available: false,
            xray_path: None,
            xray_version: None,
            assets_dir: None,
            error: Some("sidecars not found".to_string()),
        };
    };

    CoreInfo {
        available: sidecars.check().is_ok(),
        xray_path: Some(sidecars.xray().display().to_string()),
        xray_version: sidecars.version().ok(),
        assets_dir: Some(sidecars.assets_dir().display().to_string()),
        error: sidecars.check().err().map(|error| error.to_string()),
    }
}
