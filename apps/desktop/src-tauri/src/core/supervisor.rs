use crate::core::xray;
use crate::paths::AppPaths;
use crate::sidecars::Sidecars;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct CoreState {
    pub process: Option<Child>,
    pub server_id: Option<String>,
    pub server_name: Option<String>,
    pub started_at: Option<i64>,
    pub socks_port: u16,
    pub http_port: u16,
    pub error: Option<String>,
}

impl CoreState {
    pub fn new(socks_port: u16) -> Self {
        Self {
            socks_port,
            http_port: socks_port.saturating_add(1),
            ..Self::default()
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
    pub error: Option<String>,
}

pub fn status(lock: &Mutex<CoreState>, log_path: &Path) -> ConnectionStatus {
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut running = false;
    let mut pid = None;

    if let Some(child) = guard.process.as_mut() {
        match child.try_wait() {
            Ok(Some(exit)) => {
                let tail = log_tail(log_path, 25);
                guard.process = None;
                guard.error = Some(format!("Xray завершился ({exit}).\n{tail}"));
            }
            Ok(None) => {
                running = true;
                pid = Some(child.id());
            }
            Err(error) => {
                guard.error = Some(format!("не удалось проверить процесс Xray: {error}"));
            }
        }
    }

    ConnectionStatus {
        running,
        server_id: guard.server_id.clone(),
        server_name: guard.server_name.clone(),
        socks_port: guard.socks_port,
        http_port: guard.http_port,
        pid,
        started_at: guard.started_at,
        error: guard.error.clone(),
    }
}

pub fn connect(
    lock: &Mutex<CoreState>,
    paths: &AppPaths,
    sidecars: &Sidecars,
    server_id: &str,
    server_name: &str,
    mut config: serde_json::Value,
) -> Result<ConnectionStatus> {
    sidecars.check()?;

    let (socks_port, http_port) = {
        let guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        (guard.socks_port, guard.http_port)
    };

    xray::patch(
        &mut config,
        socks_port,
        http_port,
        &paths.xray_log,
        "warning",
    )?;
    xray::write_config(&config, &paths.xray_config)?;

    disconnect(lock, paths);

    let log = File::create(&paths.xray_log).context("create xray log")?;
    let stdout = log.try_clone().context("clone xray log handle")?;

    let mut command = Command::new(sidecars.xray());
    command
        .arg("run")
        .arg("-c")
        .arg(&paths.xray_config)
        .env("XRAY_LOCATION_ASSET", sidecars.assets_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log));

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

    let child = command.spawn().context("spawn xray")?;
    let pid = child.id();
    tracing::info!(pid, server = %server_name, socks_port, "xray started");

    {
        let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.process = Some(child);
        guard.server_id = Some(server_id.to_string());
        guard.server_name = Some(server_name.to_string());
        guard.started_at = Some(now_secs());
        guard.error = None;
    }

    let address = SocketAddr::from(([127, 0, 0, 1], socks_port));
    let deadline = Instant::now() + Duration::from_secs(6);

    loop {
        if let Some(error) = poll_exit(lock, &paths.xray_log) {
            return Err(anyhow!(error));
        }

        if TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok() {
            break;
        }

        if Instant::now() >= deadline {
            disconnect(lock, paths);
            let tail = log_tail(&paths.xray_log, 25);
            bail!("Xray не открыл порт {socks_port} за 6 секунд.\n{tail}");
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(status(lock, &paths.xray_log))
}

pub fn disconnect(lock: &Mutex<CoreState>, _paths: &AppPaths) -> ConnectionStatus {
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(mut child) = guard.process.take() {
        let pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        tracing::info!(pid, "xray stopped");
    }

    guard.server_id = None;
    guard.server_name = None;
    guard.started_at = None;
    guard.error = None;

    ConnectionStatus {
        running: false,
        server_id: None,
        server_name: None,
        socks_port: guard.socks_port,
        http_port: guard.http_port,
        pid: None,
        started_at: None,
        error: None,
    }
}

fn poll_exit(lock: &Mutex<CoreState>, log_path: &Path) -> Option<String> {
    let mut guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let child = guard.process.as_mut()?;
    match child.try_wait() {
        Ok(Some(exit)) => {
            let tail = log_tail(log_path, 25);
            guard.process = None;
            let message = format!("Xray завершился ({exit}).\n{tail}");
            guard.error = Some(message.clone());
            Some(message)
        }
        Ok(None) => None,
        Err(error) => {
            let message = format!("не удалось проверить процесс Xray: {error}");
            guard.error = Some(message.clone());
            Some(message)
        }
    }
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

    #[test]
    #[ignore = "requires sidecars and curl"]
    fn starts_xray_and_serves_socks() {
        let root = std::env::temp_dir().join(format!("m3vpn-core-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).expect("logs dir");
        std::fs::create_dir_all(root.join("core")).expect("core dir");

        let paths = AppPaths {
            data_dir: root.clone(),
            db_path: root.join("test.db"),
            log_dir: root.join("logs"),
            core_dir: root.join("core"),
            xray_config: root.join("core/xray.json"),
            xray_log: root.join("core/xray.log"),
        };
        let sidecars = Sidecars::discover().expect("sidecars");

        let listener = TcpListener::bind("127.0.0.1:0").expect("origin server");
        let origin_port = listener.local_addr().expect("origin address").port();
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

        let socks_port = crate::port::find_free_port(21808).expect("free socks port");
        let state = Mutex::new(CoreState::new(socks_port));
        let config = serde_json::json!({
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "listen": "0.0.0.0", "port": 10808 }
            ],
            "outbounds": [
                { "tag": "direct", "protocol": "freedom" }
            ]
        });

        let status =
            connect(&state, &paths, &sidecars, "test", "Local test", config).expect("connect xray");
        assert!(status.running);

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

        let status = disconnect(&state, &paths);
        assert!(!status.running);
        let _ = std::fs::remove_dir_all(&root);
    }
}
