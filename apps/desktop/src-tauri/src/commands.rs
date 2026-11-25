use crate::core::{self, ConnectionStatus};
use crate::cores;
use crate::ping::{self, LatencyResult, PingMode, PingOptions};
use crate::port;
use crate::routing::{self, GeoStatus, RoutingSummary};
use crate::store::{ProfileRow, ServerRow};
use crate::subscription::{self, ImportSummary};
use crate::update::{self, UpdateInfo};
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};

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
    pub helper_available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInfo {
    pub available: bool,
    pub xray_path: Option<String>,
    pub xray_version: Option<String>,
    pub singbox_available: bool,
    pub singbox_path: Option<String>,
    pub singbox_version: Option<String>,
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
        helper_available: state.helper.available(),
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
pub async fn pick_free_port(app: AppHandle, start: u16) -> Result<u16, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let start = if start == 0 { 10808 } else { start.max(1024) };
        let port = port::find_free_port(start).map_err(|error| error.to_string())?;
        {
            let mut core = state
                .core
                .lock()
                .map_err(|_| "core lock poisoned".to_string())?;
            if core.backend.is_some() {
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
        if let Some(sidecars) = state.sidecars.as_ref() {
            core::restart_front(
                &state.core,
                &state.paths,
                sidecars,
                port,
                port.saturating_add(1),
            )
            .map_err(|error| format!("{error:#}"))?;
        }
        tracing::info!(port, "selected free proxy port");
        Ok(port)
    })
    .await
    .map_err(|error| error.to_string())?
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
pub async fn import_subscription(
    app: AppHandle,
    url: String,
    name: Option<String>,
) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        subscription::import(&state.db, &url, name).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
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
pub async fn import_config(
    app: AppHandle,
    text: String,
    name: Option<String>,
) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        subscription::import_text(&state.db, &text, name).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn refresh_profile(app: AppHandle, profile_id: String) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (name, url) = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            crate::store::profile_source(&db, &profile_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "подписка не найдена".to_string())?
        };
        let url = url
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "URL подписки не сохранён — добавьте её заново".to_string())?;
        subscription::import_into(&state.db, &url, Some(name), Some(profile_id))
            .map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
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
pub async fn connect_server(app: AppHandle, server_id: String) -> Result<ConnectionStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if state.setting("mode").as_deref() == Some("tun") && !state.helper.available() {
            return Err("TUN helper не установлен: запусти scripts/install-helper.sh".to_string());
        }
        let sidecars = state
            .sidecars
            .as_ref()
            .ok_or_else(|| "ядра не найдены: запусти scripts/build-sidecars.sh".to_string())?;

        let details = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            crate::store::server_details(&db, &server_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "сервер не найден".to_string())?
        };

        let mut config: serde_json::Value = serde_json::from_str(&details.full_config_json)
            .map_err(|error| format!("битый конфиг: {error}"))?;

        let active_profile = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            routing::active(&db).map_err(|error| error.to_string())?
        };
        let singbox = m3vpn_parser::uses_singbox(&details.protocol);
        let tun_dns = active_profile
            .as_ref()
            .map(|(_, profile)| routing::tun_dns_servers(profile))
            .unwrap_or_default();
        if let Some((_, profile)) = &active_profile {
            if singbox {
                let rule_sets = routing::ensure_singbox_rule_sets(&state.paths, profile);
                routing::apply_singbox(&mut config, profile, &rule_sets)
                    .map_err(|error| format!("маршрутизация: {error:#}"))?;
            } else {
                routing::apply_xray(&mut config, profile)
                    .map_err(|error| format!("маршрутизация: {error:#}"))?;
            }
        }

        let mut status = if singbox {
            core::connect_singbox(
                &state.core,
                &state.paths,
                sidecars,
                &server_id,
                &details.name,
                config,
            )
        } else {
            core::connect_xray(
                &state.core,
                &state.paths,
                sidecars,
                &server_id,
                &details.name,
                config,
            )
        }
        .map_err(|error| format!("{error:#}"))?;

        if state.setting("mode").as_deref() == Some("tun") {
            let front_pid = core::front_pid(&state.core);
            let kill_switch = state.setting_bool("tun.kill_switch", false);
            let mut proxy_pids = Vec::new();
            if let Some(pid) = front_pid {
                proxy_pids.push(pid);
            }
            if let Some(pid) = status.pid {
                proxy_pids.push(pid);
            }
            if let Err(error) = state.helper.start_tun(
                std::process::id(),
                front_pid,
                status.socks_port,
                kill_switch,
                &tun_dns,
                &proxy_pids,
            ) {
                core::disconnect(&state.core, &state.paths);
                return Err(format!("TUN helper: {error:#}"));
            }
            state
                .tun_active
                .store(true, std::sync::atomic::Ordering::Relaxed);
            status.tun = true;
        }

        let _ = state.set_setting("last.server_id", &server_id);
        Ok(status)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn probe_servers(
    app: AppHandle,
    timeout_ms: Option<u64>,
    concurrency: Option<u32>,
    mode: Option<String>,
    test_url: Option<String>,
) -> Result<Vec<LatencyResult>, String> {
    let timeout = timeout_ms.unwrap_or(3000);
    let concurrency = concurrency.unwrap_or(4) as usize;
    let mode = PingMode::from_setting(mode.as_deref().unwrap_or("fast_http"));
    let options = PingOptions::new(mode, &test_url.unwrap_or_default(), timeout, concurrency);
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sidecars = state.sidecars.clone();
        ping::probe_all(&state.db, &options, Some(&state.paths), sidecars.as_ref())
            .map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn disconnect_core(app: AppHandle) -> Result<ConnectionStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut tun_error = None;
        if state
            .tun_active
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            if let Err(error) = state.helper.stop_tun() {
                tun_error = Some(format!("TUN helper: {error:#}"));
            }
        }
        let mut status = core::disconnect(&state.core, &state.paths);
        status.tun = false;
        match tun_error {
            Some(error) => Err(error),
            None => Ok(status),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn list_routing_profiles(state: State<'_, AppState>) -> Result<Vec<RoutingSummary>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    routing::list_profiles(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn import_routing_profile(
    app: AppHandle,
    text: String,
    name: Option<String>,
) -> Result<RoutingSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (mut profile, raw, source_url) =
            routing::parse_profile(&text).map_err(|error| format!("{error:#}"))?;
        if let Some(name) = name.filter(|name| !name.trim().is_empty()) {
            profile.name = name.trim().to_string();
        }
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let id = routing::save(&db, None, &profile, &raw, source_url)
            .map_err(|error| format!("{error:#}"))?;
        routing::list_profiles(&db)
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| "профиль не сохранился".to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_routing_profile(
    app: AppHandle,
    id: Option<String>,
    profile_json: String,
    name: Option<String>,
) -> Result<RoutingSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (mut profile, raw, source_url) =
            routing::parse_profile(&profile_json).map_err(|error| format!("{error:#}"))?;
        if let Some(name) = name.filter(|name| !name.trim().is_empty()) {
            profile.name = name.trim().to_string();
        }
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let saved = routing::save(&db, id.as_deref(), &profile, &raw, source_url)
            .map_err(|error| format!("{error:#}"))?;
        routing::list_profiles(&db)
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|item| item.id == saved)
            .ok_or_else(|| "профиль не сохранился".to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn delete_routing_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    routing::delete(&db, &id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn activate_routing_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    routing::set_active(&db, &id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn routing_rule_sets(state: State<'_, AppState>) -> Vec<routing::RuleSetStatus> {
    routing::cached_rule_sets(&state.paths)
}

#[tauri::command]
pub async fn check_core_updates(app: AppHandle) -> Result<Vec<cores::CoreVersion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sidecars = state
            .sidecars
            .as_ref()
            .ok_or_else(|| "ядра не найдены".to_string())?;
        Ok(cores::check(&state.paths, sidecars))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn update_core(app: AppHandle, kind: String) -> Result<cores::CoreVersion, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        cores::update_core(&state.paths, &kind).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn refresh_routing_geo(
    app: AppHandle,
    profile_id: Option<String>,
) -> Result<GeoStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let id = match profile_id {
            Some(id) => id,
            None => state
                .setting("routing.active_profile_id")
                .filter(|id| !id.is_empty())
                .ok_or_else(|| "активный профиль не выбран".to_string())?,
        };
        let profile = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            routing::get(&db, &id)
                .map_err(|error| error.to_string())?
                .map(|(profile, _)| profile)
                .ok_or_else(|| "профиль не найден".to_string())?
        };
        let source = state
            .setting("routing.geo_source")
            .unwrap_or_else(|| "global".to_string());
        let status = routing::update_geo(&state.paths, &profile, &source)
            .map_err(|error| format!("{error:#}"))?;
        let hours = geo_hours(&state);
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        routing::record_geo_check(
            &db,
            &id,
            status.checked_at,
            next_geo_check(status.checked_at, hours),
        )
        .map_err(|error| error.to_string())?;
        Ok(status)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn refresh_routing_geo_if_due(app: AppHandle) -> Result<Option<GeoStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let id = match state.setting("routing.active_profile_id") {
            Some(id) if !id.is_empty() => id,
            _ => return Ok(None),
        };
        let hours = geo_hours(&state);
        if hours <= 0 {
            return Ok(None);
        }
        let due = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            routing::geo_due(&db, &id, now_secs()).map_err(|error| error.to_string())?
        };
        if !due {
            return Ok(None);
        }
        let profile = {
            let db = state
                .db
                .lock()
                .map_err(|_| "database lock poisoned".to_string())?;
            routing::get(&db, &id)
                .map_err(|error| error.to_string())?
                .map(|(profile, _)| profile)
                .ok_or_else(|| "профиль не найден".to_string())?
        };
        let source = state
            .setting("routing.geo_source")
            .unwrap_or_else(|| "global".to_string());
        let status = routing::update_geo(&state.paths, &profile, &source)
            .map_err(|error| format!("{error:#}"))?;
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        routing::record_geo_check(
            &db,
            &id,
            status.checked_at,
            next_geo_check(status.checked_at, hours),
        )
        .map_err(|error| error.to_string())?;
        Ok(Some(status))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn geo_hours(state: &AppState) -> i64 {
    state
        .setting("routing.geo_update_hours")
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|hours| *hours >= 0)
        .unwrap_or(24)
}

// `0` disables automatic checks. A manual refresh still works, but must not
// make the profile look due on every startup check.
fn next_geo_check(checked_at: i64, hours: i64) -> i64 {
    if hours <= 0 {
        checked_at + 10 * 365 * 24 * 3600
    } else {
        checked_at + hours * 3600
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn check_update(app: AppHandle, repo: Option<String>) -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let configured = repo
            .filter(|value| !value.trim().is_empty())
            .or_else(|| state.setting("update.github_repo"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let Some(repo) = configured else {
            return Ok(update::empty_info());
        };
        update::check(&repo).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("разрешены только http/https ссылки".to_string());
    }
    open::that_detached(url).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_servers_excluded(
    state: State<'_, AppState>,
    server_ids: Vec<String>,
    excluded: bool,
) -> Result<(), String> {
    if server_ids.is_empty() || server_ids.len() > 5000 {
        return Err("некорректный список серверов".to_string());
    }
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    crate::store::set_servers_excluded(&db, &server_ids, excluded)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn connection_status(state: State<'_, AppState>) -> ConnectionStatus {
    let mut status = core::status(&state.core, &state.paths);
    let mut tun = state.tun_active.load(std::sync::atomic::Ordering::Relaxed);
    if tun && !state.helper.available() {
        tun = false;
        state
            .tun_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
    status.tun = status.running && tun;
    status
}

#[tauri::command]
pub async fn connection_rtt(app: AppHandle) -> Result<Option<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        core::rtt(&state.core).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn core_info(app: AppHandle) -> CoreInfo {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let Some(sidecars) = state.sidecars.as_ref() else {
            return CoreInfo {
                available: false,
                xray_path: None,
                xray_version: None,
                singbox_available: false,
                singbox_path: None,
                singbox_version: None,
                assets_dir: None,
                error: Some("sidecars not found".to_string()),
            };
        };

        let check = sidecars.check();
        let singbox_check = if sidecars.singbox().exists() {
            sidecars.check_singbox()
        } else {
            Err(anyhow::anyhow!("hiddify-sing-box is missing"))
        };
        CoreInfo {
            available: check.is_ok(),
            xray_path: Some(sidecars.xray().display().to_string()),
            xray_version: sidecars.version().ok(),
            singbox_available: singbox_check.is_ok(),
            singbox_path: Some(sidecars.singbox().display().to_string()),
            singbox_version: sidecars.singbox_version().ok(),
            assets_dir: Some(sidecars.assets_dir().display().to_string()),
            error: check.err().map(|error| error.to_string()),
        }
    })
    .await
    .unwrap_or_else(|error| CoreInfo {
        available: false,
        xray_path: None,
        xray_version: None,
        singbox_available: false,
        singbox_path: None,
        singbox_version: None,
        assets_dir: None,
        error: Some(error.to_string()),
    })
}
