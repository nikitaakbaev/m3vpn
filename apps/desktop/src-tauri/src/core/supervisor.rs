use crate::core::{front, singbox, xray};
use crate::paths::AppPaths;
use crate::sidecars::Sidecars;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DRAIN_SECONDS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Xray,
    SingBox,
}

impl Engine {
    fn id(self) -> &'static str {
        match self {
            Self::Xray => "xray",
            Self::SingBox => "sing-box",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Xray => "Xray",
            Self::SingBox => "sing-box",
        }
    }

    fn log_path(self, paths: &AppPaths) -> &Path {
        match self {
            Self::Xray => &paths.xray_log,
            Self::SingBox => &paths.singbox_log,
        }
    }
}

struct Draining {
    child: Child,
    slot: u8,
    until: i64,
}

pub struct CoreState {
    pub front: Option<Child>,
    pub front_api_port: u16,
    pub backend: Option<Child>,
    draining: Vec<Draining>,
    pub active_slot: Option<u8>,
    pub server_id: Option<String>,
    pub server_name: Option<String>,
    pub started_at: Option<i64>,
    pub socks_port: u16,
    pub http_port: u16,
    pub engine: Option<Engine>,
    pub error: Option<String>,
}

impl CoreState {
    pub fn new(socks_port: u16) -> Self {
        Self {
            front: None,
            front_api_port: 0,
            backend: None,
            draining: Vec::new(),
            active_slot: None,
            server_id: None,
            server_name: None,
            started_at: None,
            socks_port,
            http_port: socks_port.saturating_add(1),
            engine: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub running: bool,
    pub server_id: Option<String>,
    pub server_name: Option<String>,
    pub socks_port: u16,
    pub http_port: u16,
    pub pid: Option<u32>,
    pub started_at: Option<i64>,
    pub engine: Option<String>,
    pub tun: bool,
    pub error: Option<String>,
}

pub fn start_front(lock: &Mutex<CoreState>, paths: &AppPaths, sidecars: &Sidecars) -> Result<()> {
    ensure_front(lock, paths, sidecars).map(|_| ())
}

pub fn front_pid(lock: &Mutex<CoreState>) -> Option<u32> {
    let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.front.as_ref().map(Child::id)
}

pub fn status(lock: &Mutex<CoreState>, paths: &AppPaths) -> ConnectionStatus {
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    clean_draining(&mut guard);
    check_front(&mut guard, paths);
    check_backend(&mut guard, paths);

    let running = guard.backend.is_some() && guard.front.is_some();
    let pid = guard.backend.as_ref().map(Child::id);

    ConnectionStatus {
        running,
        server_id: guard.server_id.clone(),
        server_name: guard.server_name.clone(),
        socks_port: guard.socks_port,
        http_port: guard.http_port,
        pid,
        started_at: guard.started_at,
        engine: guard.engine.map(|engine| engine.id().to_string()),
        tun: false,
        error: guard.error.clone(),
    }
}

pub fn connect_xray(
    lock: &Mutex<CoreState>,
    paths: &AppPaths,
    sidecars: &Sidecars,
    server_id: &str,
    server_name: &str,
    config: serde_json::Value,
) -> Result<ConnectionStatus> {
    connect_backend(
        lock,
        paths,
        sidecars,
        server_id,
        server_name,
        config,
        Engine::Xray,
    )
}

pub fn connect_singbox(
    lock: &Mutex<CoreState>,
    paths: &AppPaths,
    sidecars: &Sidecars,
    server_id: &str,
    server_name: &str,
    config: serde_json::Value,
) -> Result<ConnectionStatus> {
    connect_backend(
        lock,
        paths,
        sidecars,
        server_id,
        server_name,
        config,
        Engine::SingBox,
    )
}

pub fn disconnect(lock: &Mutex<CoreState>, paths: &AppPaths) -> ConnectionStatus {
    let api_port = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.front_api_port
    };
    if api_port != 0 {
        for _ in 0..3 {
            if front::switch(api_port, "direct").is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut child) = guard.backend.take() {
            let pid = child.id();
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!(pid, "backend stopped");
        }
        for mut draining in guard.draining.drain(..) {
            let pid = draining.child.id();
            let _ = draining.child.kill();
            let _ = draining.child.wait();
            tracing::info!(pid, "drained backend stopped");
        }
        guard.server_id = None;
        guard.server_name = None;
        guard.started_at = None;
        guard.active_slot = None;
        guard.engine = None;
        guard.error = None;
    }

    status(lock, paths)
}

pub fn shutdown(lock: &Mutex<CoreState>, paths: &AppPaths) -> ConnectionStatus {
    let status = disconnect(lock, paths);
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(mut front) = guard.front.take() {
        let pid = front.id();
        let _ = front.kill();
        let _ = front.wait();
        tracing::info!(pid, "front proxy stopped");
    }
    guard.front_api_port = 0;
    status
}

pub fn restart_front(
    lock: &Mutex<CoreState>,
    paths: &AppPaths,
    sidecars: &Sidecars,
    socks_port: u16,
    http_port: u16,
) -> Result<()> {
    {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut front) = guard.front.take() {
            let _ = front.kill();
            let _ = front.wait();
        }
        guard.front_api_port = 0;
        guard.socks_port = socks_port;
        guard.http_port = http_port;
    }
    ensure_front(lock, paths, sidecars).map(|_| ())
}

pub fn rtt(lock: &Mutex<CoreState>) -> Result<Option<u64>> {
    let (running, socks_port) = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            guard.backend.is_some() && guard.front.is_some(),
            guard.socks_port,
        )
    };
    if !running {
        return Ok(None);
    }

    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks_port}"))
        .context("build SOCKS proxy")?;
    let client = reqwest::blocking::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(6))
        .build()
        .context("build RTT client")?;

    let started = Instant::now();
    let response = client
        .get("https://www.gstatic.com/generate_204")
        .send()
        .context("RTT request")?;
    if !response.status().is_success() {
        bail!("RTT probe returned {}", response.status());
    }
    Ok(Some(started.elapsed().as_millis() as u64))
}

fn connect_backend(
    lock: &Mutex<CoreState>,
    paths: &AppPaths,
    sidecars: &Sidecars,
    server_id: &str,
    server_name: &str,
    mut config: serde_json::Value,
    engine: Engine,
) -> Result<ConnectionStatus> {
    let api_port = ensure_front(lock, paths, sidecars)?;
    let socks_port = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.socks_port
    };

    let slot = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.active_slot {
            Some(0) => 1,
            Some(_) => 0,
            None if guard.backend.is_some() => 1,
            None => 0,
        }
    };
    kill_draining_slot(lock, slot);

    let slot_socks = slot_port(socks_port, slot);
    let slot_http = slot_socks.saturating_add(1);
    let (binary, config_path, log_path, extra_env) = match engine {
        Engine::Xray => {
            sidecars.check()?;
            let assets = if paths.geo_dir.join("geoip.dat").exists()
                && paths.geo_dir.join("geosite.dat").exists()
            {
                paths.geo_dir.clone()
            } else {
                sidecars.assets_dir()
            };
            xray::patch(
                &mut config,
                slot_socks,
                slot_http,
                &paths.xray_log,
                "warning",
            )?;
            xray::write_config(&config, &paths.xray_config)?;
            (
                sidecars.xray(),
                paths.xray_config.clone(),
                paths.xray_log.clone(),
                Some(("XRAY_LOCATION_ASSET", assets)),
            )
        }
        Engine::SingBox => {
            sidecars.check_singbox()?;
            let api = crate::port::find_free_port(slot_http.saturating_add(50))
                .context("find free backend API port")?;
            singbox::patch(&mut config, slot_socks, api)?;
            singbox::write_config(&config, &paths.singbox_config)?;
            (
                sidecars.singbox(),
                paths.singbox_config.clone(),
                paths.singbox_log.clone(),
                None,
            )
        }
    };

    let mut child = spawn(&binary, &config_path, &log_path, extra_env)?;
    if let Err(error) = wait_backend(&mut child, engine, &log_path, slot_socks) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    if let Err(error) = front::switch(api_port, slot_tag(slot)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.context("switch front proxy to new backend"));
    }

    let old = {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let old = guard.backend.take().map(|child| (child, guard.active_slot));
        guard.backend = Some(child);
        guard.active_slot = Some(slot);
        guard.server_id = Some(server_id.to_string());
        guard.server_name = Some(server_name.to_string());
        guard.started_at = Some(now_secs());
        guard.engine = Some(engine);
        guard.error = None;
        old
    };

    match old {
        Some((old_child, Some(old_slot))) => {
            let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.draining.push(Draining {
                child: old_child,
                slot: old_slot,
                until: now_secs() + DRAIN_SECONDS,
            });
            tracing::info!(slot = old_slot, "old backend is draining");
        }
        Some((mut old_child, None)) => {
            let _ = old_child.kill();
            let _ = old_child.wait();
        }
        None => {}
    }

    let pid = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.backend.as_ref().map(Child::id)
    };
    tracing::info!(pid, server = %server_name, slot, "backend started through front proxy");

    Ok(status(lock, paths))
}

fn ensure_front(lock: &Mutex<CoreState>, paths: &AppPaths, sidecars: &Sidecars) -> Result<u16> {
    {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let api_port = guard.front_api_port;
        if let Some(front) = guard.front.as_mut() {
            match front.try_wait() {
                Ok(None) if api_port != 0 => return Ok(api_port),
                Ok(None) => {
                    let _ = front.kill();
                    let _ = front.wait();
                    guard.front = None;
                    guard.front_api_port = 0;
                }
                Ok(Some(_)) => {
                    guard.front = None;
                    guard.front_api_port = 0;
                }
                Err(error) => {
                    guard.front = None;
                    guard.front_api_port = 0;
                    guard.error = Some(format!("front proxy check failed: {error}"));
                }
            }
        }
    }

    sidecars.check_singbox()?;
    let (socks_port, http_port) = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        (guard.socks_port, guard.http_port)
    };
    let api_port = crate::port::find_free_port(socks_port.saturating_add(500))
        .context("find free front proxy API port")?;
    let slot_a = slot_port(socks_port, 0);
    let slot_b = slot_port(socks_port, 1);
    let config = front::config(socks_port, http_port, api_port, slot_a, slot_b);
    front::write_config(&config, &paths.front_config)?;

    let mut child = spawn(
        &sidecars.singbox(),
        &paths.front_config,
        &paths.front_log,
        None,
    )?;
    if let Err(error) = wait_front(&mut child, &paths.front_log, socks_port, api_port) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.front = Some(child);
        guard.front_api_port = api_port;
    }
    front::switch(api_port, "direct").context("initialize front proxy selector")?;
    tracing::info!(socks_port, api_port, "front proxy started");
    Ok(api_port)
}

fn slot_port(base: u16, slot: u8) -> u16 {
    base.saturating_add(1000)
        .saturating_add(u16::from(slot) * 10)
}

fn slot_tag(slot: u8) -> &'static str {
    if slot == 0 {
        "upstream-a"
    } else {
        "upstream-b"
    }
}

fn kill_draining_slot(lock: &Mutex<CoreState>, slot: u8) {
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut kept = Vec::new();
    for mut draining in guard.draining.drain(..) {
        if draining.slot == slot {
            let pid = draining.child.id();
            let _ = draining.child.kill();
            let _ = draining.child.wait();
            tracing::info!(pid, slot, "draining backend stopped to reuse slot");
        } else {
            kept.push(draining);
        }
    }
    guard.draining = kept;
}

fn clean_draining(guard: &mut CoreState) {
    let now = now_secs();
    let mut kept = Vec::new();
    for mut draining in guard.draining.drain(..) {
        if now >= draining.until {
            let pid = draining.child.id();
            let _ = draining.child.kill();
            let _ = draining.child.wait();
            tracing::info!(pid, slot = draining.slot, "drain timeout, backend stopped");
        } else {
            kept.push(draining);
        }
    }
    guard.draining = kept;
}

fn check_front(guard: &mut CoreState, paths: &AppPaths) {
    let Some(front) = guard.front.as_mut() else {
        guard.error = Some("front proxy is not running".to_string());
        return;
    };
    match front.try_wait() {
        Ok(Some(exit)) => {
            let tail = log_tail(&paths.front_log, 25);
            guard.front = None;
            guard.front_api_port = 0;
            if let Some(mut child) = guard.backend.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            for mut draining in guard.draining.drain(..) {
                let _ = draining.child.kill();
                let _ = draining.child.wait();
            }
            guard.server_id = None;
            guard.server_name = None;
            guard.started_at = None;
            guard.active_slot = None;
            guard.engine = None;
            guard.error = Some(format!("front proxy завершился ({exit}).\n{tail}"));
        }
        Ok(None) => {}
        Err(error) => {
            guard.error = Some(format!("front proxy check failed: {error}"));
        }
    }
}

fn check_backend(guard: &mut CoreState, paths: &AppPaths) {
    let Some(child) = guard.backend.as_mut() else {
        return;
    };
    let engine = guard.engine.unwrap_or(Engine::Xray);
    let log_path = engine.log_path(paths);
    match child.try_wait() {
        Ok(Some(exit)) => {
            let tail = log_tail(log_path, 25);
            guard.backend = None;
            guard.server_id = None;
            guard.server_name = None;
            guard.started_at = None;
            guard.active_slot = None;
            guard.engine = None;
            guard.error = Some(format!("{} завершился ({exit}).\n{tail}", engine.label()));
            if guard.front_api_port != 0 {
                let _ = front::switch(guard.front_api_port, "direct");
            }
        }
        Ok(None) => {}
        Err(error) => {
            guard.error = Some(format!(
                "не удалось проверить процесс {}: {error}",
                engine.label()
            ));
        }
    }
}

fn wait_front(child: &mut Child, log_path: &Path, socks_port: u16, api_port: u16) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Ok(Some(exit)) = child.try_wait() {
            let tail = log_tail(log_path, 25);
            bail!("front proxy завершился ({exit}).\n{tail}");
        }
        if port_open(socks_port) && port_open(api_port) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let tail = log_tail(log_path, 25);
            bail!("front proxy не открыл порты за 6 секунд.\n{tail}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_backend(child: &mut Child, engine: Engine, log_path: &Path, port: u16) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Ok(Some(exit)) = child.try_wait() {
            let tail = log_tail(log_path, 25);
            bail!("{} завершился ({exit}).\n{tail}", engine.label());
        }
        if port_open(port) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let tail = log_tail(log_path, 25);
            bail!(
                "{} не открыл порт {port} за 6 секунд.\n{tail}",
                engine.label()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn port_open(port: u16) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok()
}

fn spawn(
    binary: &Path,
    config: &Path,
    log_path: &Path,
    extra_env: Option<(&str, std::path::PathBuf)>,
) -> Result<Child> {
    let log = File::create(log_path).with_context(|| format!("create {}", log_path.display()))?;
    let stdout = log.try_clone().context("clone core log handle")?;

    let mut command = Command::new(binary);
    command
        .arg("run")
        .arg("-c")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log));
    if let Some((key, value)) = extra_env {
        command.env(key, value);
    }

    // If the UI is killed with SIGKILL, take the core down with it instead of
    // leaving an orphan process bound to the local proxy port.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }

    // Spawn from a keeper thread that stays alive while the child lives.
    // PR_SET_PDEATHSIG is tied to the parent *thread*; spawning directly from
    // a short-lived command thread made Xray die as soon as connect returned.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = command.spawn();
        let pid = result.as_ref().ok().map(|child| child.id());
        if tx.send(result).is_err() {
            return;
        }
        #[cfg(target_os = "linux")]
        if let Some(pid) = pid {
            let proc_entry = format!("/proc/{pid}");
            while Path::new(&proc_entry).exists() {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = pid;
            std::thread::park();
        }
    });

    let result = rx
        .recv()
        .map_err(|_| anyhow::anyhow!("core spawner thread died"))?;
    result.map_err(|error| anyhow::anyhow!("spawn {}: {error}", binary.display()))
}

pub fn log_tail(path: &Path, lines: usize) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let start = length.saturating_sub(16 * 1024);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buffer = String::new();
    if file.read_to_string(&mut buffer).is_err() {
        return String::new();
    }
    let collected: Vec<&str> = buffer.lines().rev().take(lines).collect();
    collected.into_iter().rev().collect::<Vec<_>>().join("\n")
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use crate::sidecars::Sidecars;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn test_paths(name: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!("m3vpn-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).expect("logs dir");
        std::fs::create_dir_all(root.join("core")).expect("core dir");
        AppPaths {
            data_dir: root.clone(),
            db_path: root.join("test.db"),
            log_dir: root.join("logs"),
            core_dir: root.join("core"),
            xray_config: root.join("core/xray.json"),
            xray_log: root.join("core/xray.log"),
            singbox_config: root.join("core/singbox.json"),
            singbox_log: root.join("core/singbox.log"),
            front_config: root.join("core/front.json"),
            front_log: root.join("core/front.log"),
            geo_dir: root.join("geo"),
        }
    }

    fn origin_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("origin server");
        let port = listener.local_addr().expect("origin address").port();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });
        port
    }

    fn slow_origin(size: usize) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("origin server");
        let port = listener.local_addr().expect("origin address").port();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buffer = [0u8; 2048];
                let _ = stream.read(&mut buffer);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n"
                );
                if stream.write_all(header.as_bytes()).is_err() {
                    continue;
                }
                let body = vec![b'x'; size];
                let _ = stream.write_all(&body);
            }
        });
        port
    }

    fn direct_config() -> serde_json::Value {
        serde_json::json!({
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "listen": "127.0.0.1", "port": 10808 }
            ],
            "outbounds": [
                { "tag": "direct", "protocol": "freedom" }
            ]
        })
    }

    fn direct_singbox_config() -> serde_json::Value {
        serde_json::json!({
            "inbounds": [
                { "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 10808 }
            ],
            "outbounds": [
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        })
    }

    fn curl_through(socks_port: u16, origin_port: u16) -> String {
        let output = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("10")
            .arg("--socks5-hostname")
            .arg(format!("127.0.0.1:{socks_port}"))
            .arg(format!("http://127.0.0.1:{origin_port}/"))
            .output()
            .expect("run curl");
        assert!(
            output.status.success(),
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn backend_survives_short_lived_connect_thread() {
        use std::sync::Arc;

        let paths = test_paths("keeper-test");
        let sidecars = Sidecars::discover().expect("sidecars");
        let origin_port = origin_server();
        let socks_port = crate::port::find_free_port(25008).expect("free socks port");
        let state = Arc::new(Mutex::new(CoreState::new(socks_port)));

        {
            let state = Arc::clone(&state);
            let paths = paths.clone();
            let sidecars = sidecars.clone();
            std::thread::spawn(move || {
                connect_xray(&state, &paths, &sidecars, "a", "Server A", direct_config())
                    .expect("connect in short-lived thread");
            })
            .join()
            .expect("connect thread");
        }

        std::thread::sleep(Duration::from_secs(2));
        let current = status(&state, &paths);
        assert!(current.running, "backend died with the command thread");

        let output = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("10")
            .arg("--socks5-hostname")
            .arg(format!("127.0.0.1:{socks_port}"))
            .arg(format!("http://127.0.0.1:{origin_port}/"))
            .output()
            .expect("run curl");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");

        shutdown(&state, &paths);
        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn failed_backend_falls_back_to_direct() {
        let paths = test_paths("fail-open-test");
        let sidecars = Sidecars::discover().expect("sidecars");
        let origin_port = origin_server();
        let socks_port = crate::port::find_free_port(27008).expect("free socks port");
        let state = Mutex::new(CoreState::new(socks_port));

        let connected = connect_xray(&state, &paths, &sidecars, "a", "Server A", direct_config())
            .expect("connect");
        let pid = connected.pid.expect("backend pid");

        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
        std::thread::sleep(Duration::from_millis(300));

        let failed = status(&state, &paths);
        assert!(!failed.running);
        assert!(failed.error.is_some());

        let output = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("10")
            .arg("--socks5-hostname")
            .arg(format!("127.0.0.1:{socks_port}"))
            .arg(format!("http://127.0.0.1:{origin_port}/"))
            .output()
            .expect("run curl");
        assert!(
            output.status.success(),
            "front proxy did not fail open: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");

        shutdown(&state, &paths);
        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn hot_switch_keeps_an_active_download() {
        let paths = test_paths("hot-switch-test");
        let sidecars = Sidecars::discover().expect("sidecars");
        let size = 4 * 1024 * 1024;
        let origin_port = slow_origin(size);
        let socks_port = crate::port::find_free_port(23008).expect("free socks port");
        let state = Mutex::new(CoreState::new(socks_port));

        connect_xray(&state, &paths, &sidecars, "a", "Server A", direct_config())
            .expect("connect A");

        let target = paths.data_dir.join("download.bin");
        let mut curl = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("60")
            .arg("--limit-rate")
            .arg("300k")
            .arg("--socks5-hostname")
            .arg(format!("127.0.0.1:{socks_port}"))
            .arg(format!("http://127.0.0.1:{origin_port}/"))
            .arg("-o")
            .arg(&target)
            .spawn()
            .expect("spawn curl");

        std::thread::sleep(Duration::from_secs(1));
        assert!(curl.try_wait().expect("curl status").is_none());

        connect_xray(&state, &paths, &sidecars, "b", "Server B", direct_config())
            .expect("connect B");

        let status = curl.wait().expect("wait curl");
        assert!(status.success(), "download was interrupted");
        assert_eq!(
            std::fs::metadata(&target).expect("download metadata").len() as usize,
            size
        );

        let status = shutdown(&state, &paths);
        assert!(!status.running);
        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn switches_between_xray_and_singbox_backends() {
        let paths = test_paths("engine-switch-test");
        let sidecars = Sidecars::discover().expect("sidecars");
        let origin_port = origin_server();
        let socks_port = crate::port::find_free_port(29008).expect("free socks port");
        let state = Mutex::new(CoreState::new(socks_port));

        let status = connect_xray(&state, &paths, &sidecars, "a", "Xray A", direct_config())
            .expect("connect xray");
        assert_eq!(status.engine.as_deref(), Some("xray"));
        assert_eq!(curl_through(socks_port, origin_port), "ok");

        let status = connect_singbox(
            &state,
            &paths,
            &sidecars,
            "b",
            "Sing-box B",
            direct_singbox_config(),
        )
        .expect("connect sing-box");
        assert_eq!(status.engine.as_deref(), Some("sing-box"));
        assert_eq!(curl_through(socks_port, origin_port), "ok");

        let status = connect_xray(&state, &paths, &sidecars, "c", "Xray C", direct_config())
            .expect("connect xray again");
        assert_eq!(status.engine.as_deref(), Some("xray"));
        assert_eq!(curl_through(socks_port, origin_port), "ok");

        shutdown(&state, &paths);
        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn starts_xray_and_serves_socks() {
        let paths = test_paths("core-test");
        let sidecars = Sidecars::discover().expect("sidecars");
        let origin_port = origin_server();
        let socks_port = crate::port::find_free_port(21008).expect("free socks port");
        let state = Mutex::new(CoreState::new(socks_port));
        let config = serde_json::json!({
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "listen": "0.0.0.0", "port": 10808 }
            ],
            "outbounds": [
                { "tag": "direct", "protocol": "freedom" }
            ]
        });

        let status = connect_xray(&state, &paths, &sidecars, "test", "Local test", config)
            .expect("connect xray");
        assert!(status.running);
        assert_eq!(status.engine.as_deref(), Some("xray"));

        let output = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("10")
            .arg("--socks5-hostname")
            .arg(format!("127.0.0.1:{socks_port}"))
            .arg(format!("http://127.0.0.1:{origin_port}/"))
            .output()
            .expect("run curl");
        assert!(
            output.status.success(),
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");

        let rtt = rtt(&state).expect("rtt").expect("running");
        assert!(rtt > 0);

        let status = shutdown(&state, &paths);
        assert!(!status.running);
        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }
}
