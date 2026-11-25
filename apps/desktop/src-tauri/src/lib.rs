mod commands;
mod core;
mod helper_client;
mod logging;
mod paths;
mod ping;
mod port;
mod routing;
mod sidecars;
mod store;
mod subscription;
mod tray;
mod update;

use core::CoreState;
use helper_client::HelperClient;
use logging::LoggingGuard;
use paths::AppPaths;
use rusqlite::Connection;
use sidecars::Sidecars;
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::Mutex;
use tauri::{Manager, WindowEvent};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub paths: AppPaths,
    pub sidecars: Option<Sidecars>,
    pub core: Mutex<CoreState>,
    pub helper: HelperClient,
    pub tun_active: AtomicBool,
    pub _logging: Mutex<Option<LoggingGuard>>,
    pub proxy_port: AtomicU16,
}

impl AppState {
    pub fn setting(&self, key: &str) -> Option<String> {
        let db = self.db.lock().ok()?;
        store::get(&db, key).ok().flatten()
    }

    pub fn setting_bool(&self, key: &str, fallback: bool) -> bool {
        match self.setting(key).as_deref() {
            Some("true") | Some("1") | Some("yes") | Some("on") => true,
            Some("false") | Some("0") | Some("no") | Some("off") => false,
            _ => fallback,
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
        store::set(&db, key, value)
    }
}

pub fn run() {
    #[cfg(unix)]
    unsafe {
        // Keep databases, logs and WAL files private to the user.
        libc::umask(0o077);
    }

    let paths = match AppPaths::new() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("failed to resolve application paths: {error:#}");
            std::process::exit(1);
        }
    };
    if let Err(error) = paths.ensure() {
        eprintln!("failed to create application directories: {error:#}");
        std::process::exit(1);
    }

    let logging = match logging::init(&paths.log_dir) {
        Ok(logging) => logging,
        Err(error) => {
            eprintln!("failed to initialize logging: {error:#}");
            std::process::exit(1);
        }
    };

    let conn = match store::open(&paths.db_path) {
        Ok(conn) => conn,
        Err(error) => {
            eprintln!("failed to open database: {error:#}");
            std::process::exit(1);
        }
    };

    let base_port = store::get(&conn, "proxy.port")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(10808);
    let auto_port = store::get(&conn, "proxy.auto_port")
        .ok()
        .flatten()
        .map(|value| value != "false")
        .unwrap_or(true);
    let active_port = if auto_port {
        port::find_free_port(base_port).unwrap_or(base_port)
    } else {
        base_port
    };
    let _ = store::set(&conn, "proxy.active_port", &active_port.to_string());

    let sidecars = match Sidecars::discover() {
        Ok(sidecars) => {
            if let Err(error) = sidecars.check() {
                tracing::warn!(%error, "sidecars are incomplete");
            }
            Some(sidecars)
        }
        Err(error) => {
            tracing::warn!(%error, "sidecars are unavailable; connect will fail");
            None
        }
    };

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %paths.data_dir.display(),
        port = active_port,
        "m3vpn starting"
    );

    let state = AppState {
        db: Mutex::new(conn),
        paths,
        sidecars,
        core: Mutex::new(CoreState::new(active_port)),
        helper: HelperClient::new(),
        tun_active: AtomicBool::new(false),
        _logging: Mutex::new(Some(logging)),
        proxy_port: AtomicU16::new(active_port),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app);
        }))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_settings,
            commands::set_setting,
            commands::pick_free_port,
            commands::open_logs_dir,
            commands::log_frontend,
            commands::import_subscription,
            commands::import_config,
            commands::list_profiles,
            commands::list_servers,
            commands::delete_profile,
            commands::refresh_profile,
            commands::probe_servers,
            commands::connect_server,
            commands::disconnect_core,
            commands::connection_status,
            commands::connection_rtt,
            commands::core_info,
            commands::open_core_dir,
            commands::list_routing_profiles,
            commands::import_routing_profile,
            commands::save_routing_profile,
            commands::delete_routing_profile,
            commands::activate_routing_profile,
            commands::refresh_routing_geo,
            commands::refresh_routing_geo_if_due,
            commands::check_update,
            commands::open_external,
        ])
        .setup(|app| {
            {
                let state = app.state::<AppState>();
                if let Some(sidecars) = state.sidecars.as_ref() {
                    if let Err(error) = routing::ensure_seed_geo(&state.paths, sidecars) {
                        tracing::warn!(%error, "failed to seed geo assets");
                    }
                    if let Err(error) = core::start_front(&state.core, &state.paths, sidecars) {
                        tracing::warn!(%error, "front proxy failed to start");
                    }
                }
            }
            tray::setup(app)?;

            if app
                .state::<AppState>()
                .setting_bool("startup.minimized", false)
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            tracing::info!("application ready");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if state.setting_bool("startup.close_to_tray", true) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            shutdown_core(app_handle);
        }
    });
}

pub fn cli_import(url: &str, name: Option<String>) -> anyhow::Result<subscription::ImportSummary> {
    let paths = AppPaths::new()?;
    paths.ensure()?;
    let conn = store::open(&paths.db_path)?;
    subscription::import(&Mutex::new(conn), url, name)
}

pub fn cli_probe() -> anyhow::Result<(usize, usize)> {
    let paths = AppPaths::new()?;
    paths.ensure()?;
    let conn = store::open(&paths.db_path)?;
    let mode = ping::PingMode::from_setting(
        &store::get(&conn, "ping.protocol")?.unwrap_or_else(|| "fast_http".to_string()),
    );
    let test_url = store::get(&conn, "ping.url")?.unwrap_or_default();
    let timeout = store::get(&conn, "ping.timeout_ms")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(3000);
    let concurrency = store::get(&conn, "ping.concurrency")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);
    let options = ping::PingOptions::new(mode, &test_url, timeout, concurrency);
    let sidecars = Sidecars::discover().ok();
    let results = ping::probe_all(&Mutex::new(conn), &options, Some(&paths), sidecars.as_ref())?;
    let ok = results.iter().filter(|result| result.ok).count();
    Ok((ok, results.len()))
}

pub fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn shutdown_core(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if state
        .tun_active
        .swap(false, std::sync::atomic::Ordering::Relaxed)
    {
        if let Err(error) = state.helper.stop_tun() {
            tracing::warn!(%error, "failed to stop TUN helper");
        }
    }
    core::shutdown(&state.core, &state.paths);
}
