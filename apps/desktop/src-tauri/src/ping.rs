use crate::core::{singbox, xray};
use crate::paths::AppPaths;
use crate::sidecars::Sidecars;
use crate::store::{self, ServerRow};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use rusqlite::Connection;
use serde::Serialize;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_TEST_URL: &str = "https://www.gstatic.com/generate_204";
const FAST_HTTP_SCALE: f64 = 3.3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PingMode {
    FastHttp,
    Tcp,
    ProxyGet,
    ProxyHead,
}

impl PingMode {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "tcp" => Self::Tcp,
            "proxy_get" => Self::ProxyGet,
            "proxy_head" => Self::ProxyHead,
            _ => Self::FastHttp,
        }
    }

    fn uses_proxy(self) -> bool {
        !matches!(self, Self::Tcp)
    }
}

#[derive(Clone, Debug)]
pub struct PingOptions {
    pub mode: PingMode,
    pub test_url: String,
    pub timeout: Duration,
    pub concurrency: usize,
}

impl PingOptions {
    pub fn new(mode: PingMode, test_url: &str, timeout_ms: u64, concurrency: usize) -> Self {
        let test_url = test_url.trim();
        Self {
            mode,
            test_url: if test_url.is_empty() {
                DEFAULT_TEST_URL.to_string()
            } else {
                test_url.to_string()
            },
            timeout: Duration::from_millis(timeout_ms.clamp(500, 10_000)),
            concurrency,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyResult {
    pub server_id: String,
    pub latency_ms: Option<u64>,
    pub ok: bool,
    pub error: Option<String>,
}

pub fn probe_all(
    db: &Mutex<Connection>,
    options: &PingOptions,
    paths: Option<&AppPaths>,
    sidecars: Option<&Sidecars>,
) -> Result<Vec<LatencyResult>> {
    if options.mode.uses_proxy() && (paths.is_none() || sidecars.is_none()) {
        bail!("для HTTP-пинга не найдены ядра");
    }

    let servers = {
        let db = db.lock().map_err(|_| anyhow!("database lock poisoned"))?;
        store::list_servers(&db, None)?
            .into_iter()
            .filter(|server| server.enabled)
            .collect::<Vec<_>>()
    };

    let concurrency = if options.mode.uses_proxy() {
        options.concurrency.clamp(1, 8)
    } else {
        options.concurrency.clamp(1, 16)
    }
    .min(servers.len().max(1));
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(servers.len()));

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(server) = servers.get(index) else {
                    break;
                };
                let result = if options.mode.uses_proxy() {
                    probe_proxy(index, server, options, paths, sidecars, db)
                } else {
                    probe_tcp(server, options.timeout)
                };
                if let Ok(mut results) = results.lock() {
                    results.push(result);
                }
            });
        }
    });

    let mut results = results
        .into_inner()
        .map_err(|_| anyhow!("ping result lock poisoned"))?;
    results.sort_by(|left, right| left.server_id.cmp(&right.server_id));

    {
        let db = db.lock().map_err(|_| anyhow!("database lock poisoned"))?;
        let now = now_secs();
        for result in &results {
            store::update_latency(&db, &result.server_id, result.latency_ms, result.ok, now)?;
        }
    }

    Ok(results)
}

fn probe_tcp(server: &ServerRow, timeout: Duration) -> LatencyResult {
    let Some(host) = server.host.as_deref() else {
        return failure(server, "нет адреса");
    };
    let Some(port) = server.port else {
        return failure(server, "нет порта");
    };

    let addresses = match (host, port).to_socket_addrs() {
        Ok(addresses) => addresses.collect::<Vec<_>>(),
        Err(error) => return failure(server, format!("DNS: {error}")),
    };
    if addresses.is_empty() {
        return failure(server, "DNS вернул пустой список");
    }

    let mut last_error = None;
    for address in addresses {
        let started = Instant::now();
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(_) => {
                return LatencyResult {
                    server_id: server.id.clone(),
                    latency_ms: Some(started.elapsed().as_millis() as u64),
                    ok: true,
                    error: None,
                };
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    failure(
        server,
        last_error.unwrap_or_else(|| "не удалось подключиться".to_string()),
    )
}

fn probe_proxy(
    index: usize,
    server: &ServerRow,
    options: &PingOptions,
    paths: Option<&AppPaths>,
    sidecars: Option<&Sidecars>,
    db: &Mutex<Connection>,
) -> LatencyResult {
    let (Some(paths), Some(sidecars)) = (paths, sidecars) else {
        return failure(server, "ядра не найдены");
    };
    let details = {
        let db = match db.lock() {
            Ok(db) => db,
            Err(_) => return failure(server, "database lock poisoned"),
        };
        match store::server_details(&db, &server.id) {
            Ok(Some(details)) => details,
            Ok(None) => return failure(server, "сервер не найден"),
            Err(error) => return failure(server, error.to_string()),
        }
    };
    let config = match serde_json::from_str(&details.full_config_json) {
        Ok(config) => config,
        Err(error) => return failure(server, format!("битый конфиг: {error}")),
    };

    let started = Instant::now();
    let core = match ProbeCore::start(index, paths, sidecars, &details.protocol, config) {
        Ok(core) => core,
        Err(error) => return failure(server, format!("{error:#}")),
    };
    let http = http_latency(
        core.socks_port,
        &options.test_url,
        options.mode,
        options.timeout,
    );
    let startup_and_http = started.elapsed();
    drop(core);

    match http {
        Ok(latency) => {
            let latency = if options.mode == PingMode::FastHttp {
                ((latency as f64) / FAST_HTTP_SCALE).round().max(1.0) as u64
            } else {
                latency
            };
            tracing::debug!(
                server = %server.name,
                mode = ?options.mode,
                http_ms = latency,
                total_ms = startup_and_http.as_millis() as u64,
                "proxy ping"
            );
            LatencyResult {
                server_id: server.id.clone(),
                latency_ms: Some(latency),
                ok: true,
                error: None,
            }
        }
        Err(error) => failure(server, format!("{error:#}")),
    }
}

fn http_latency(socks_port: u16, url: &str, mode: PingMode, timeout: Duration) -> Result<u64> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks_port}"))
        .context("build SOCKS proxy")?;
    let client = Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build HTTP client")?;

    let started = Instant::now();
    let response = if mode == PingMode::ProxyHead {
        client.head(url).send().context("HTTP HEAD через прокси")?
    } else {
        client.get(url).send().context("HTTP GET через прокси")?
    };
    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status}");
    }
    if mode != PingMode::ProxyHead {
        let _ = response.bytes().context("read HTTP response")?;
    }
    Ok(started.elapsed().as_millis() as u64)
}

struct ProbeCore {
    child: Child,
    socks_port: u16,
    config: PathBuf,
    log: PathBuf,
}

impl ProbeCore {
    fn start(
        index: usize,
        paths: &AppPaths,
        sidecars: &Sidecars,
        protocol: &str,
        mut config: serde_json::Value,
    ) -> Result<Self> {
        let (socks_port, second_port) = two_free_ports()?;
        let config_path = paths
            .core_dir
            .join(format!("probe-{}-{index}.json", std::process::id()));
        let log_path = paths
            .core_dir
            .join(format!("probe-{}-{index}.log", std::process::id()));
        let singbox = m3vpn_parser::uses_singbox(protocol);
        let assets = sidecars.assets_dir();

        let (binary, extra_env) = if singbox {
            sidecars.check_singbox()?;
            singbox::patch(&mut config, socks_port, second_port)?;
            singbox::write_config(&config, &config_path)?;
            (sidecars.singbox(), None)
        } else {
            sidecars.check()?;
            let assets = if paths.geo_dir.join("geoip.dat").exists()
                && paths.geo_dir.join("geosite.dat").exists()
            {
                paths.geo_dir.clone()
            } else {
                assets
            };
            xray::patch(&mut config, socks_port, second_port, &log_path, "warning")?;
            xray::write_config(&config, &config_path)?;
            (sidecars.xray(), Some(("XRAY_LOCATION_ASSET", assets)))
        };

        let log = std::fs::File::create(&log_path)
            .with_context(|| format!("create {}", log_path.display()))?;
        let stdout = log.try_clone().context("clone probe log")?;
        let mut command = Command::new(&binary);
        command
            .arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log));
        if let Some((key, value)) = extra_env {
            command.env(key, value);
        }
        #[cfg(target_os = "linux")]
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }

        let mut child = command.spawn().context("spawn probe core")?;
        if let Err(error) = wait_for_port(&mut child, socks_port, &log_path) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&config_path);
            let _ = std::fs::remove_file(&log_path);
            return Err(error);
        }
        Ok(Self {
            child,
            socks_port,
            config: config_path,
            log: log_path,
        })
    }
}

impl Drop for ProbeCore {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.config);
        let _ = std::fs::remove_file(&self.log);
    }
}

fn two_free_ports() -> Result<(u16, u16)> {
    let first = TcpListener::bind(("127.0.0.1", 0)).context("bind probe port")?;
    let second = TcpListener::bind(("127.0.0.1", 0)).context("bind probe port")?;
    let first_port = first.local_addr()?.port();
    let second_port = second.local_addr()?.port();
    drop(first);
    drop(second);
    Ok((first_port, second_port))
}

fn wait_for_port(child: &mut Child, port: u16, log: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Ok(Some(exit)) = child.try_wait() {
            bail!("ядро пинга завершилось ({exit}).\n{}", log_tail(log, 12));
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("ядро пинга не открыло порт {port}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn log_tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut tail = text.lines().rev().take(lines).collect::<Vec<_>>();
    tail.reverse();
    tail.join("\n")
}

fn failure(server: &ServerRow, error: impl Into<String>) -> LatencyResult {
    LatencyResult {
        server_id: server.id.clone(),
        latency_ms: None,
        ok: false,
        error: Some(error.into()),
    }
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
    use crate::store::{self, NewProfile, NewServer};
    use std::net::TcpListener;

    fn tcp_server(port: u16) -> (Mutex<Connection>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("m3vpn-ping-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = store::open(&path).expect("open test database");
        let profile = NewProfile {
            id: "p1".to_string(),
            name: "Test".to_string(),
            url: "http://example.test".to_string(),
            url_hidden: false,
            title: None,
            description: None,
            support_url: None,
            web_page_url: None,
            update_interval_hours: 24,
            userinfo: None,
            created_at: 0,
            updated_at: 0,
        };
        let server = NewServer {
            id: "p1:0".to_string(),
            profile_id: "p1".to_string(),
            name: "Local".to_string(),
            protocol: "socks".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(port),
            params_json: None,
            full_config_json: "{}".to_string(),
            sort_index: 0,
        };
        store::save_import(&conn, &profile, &[server]).expect("save import");
        (Mutex::new(conn), path)
    }

    #[test]
    fn probes_a_local_port_and_persists_latency() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("listener address").port();
        std::thread::spawn(move || for _ in listener.incoming() {});

        let (db, path) = tcp_server(port);
        let options = PingOptions::new(PingMode::Tcp, "", 1000, 2);
        let results = probe_all(&db, &options, None, None).expect("probe");
        assert_eq!(results.len(), 1);
        assert!(results[0].ok);
        assert!(results[0].latency_ms.is_some());

        let servers = store::list_servers(&db.lock().unwrap(), None).expect("list");
        assert!(servers[0].latency_ms.is_some());

        drop(db);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }

    #[test]
    fn fast_http_scale_reads_like_a_familiar_ping() {
        assert_eq!((330.0 / FAST_HTTP_SCALE).round(), 100.0);
    }

    #[test]
    #[ignore = "requires sidecars"]
    fn fast_http_ping_makes_an_http_request_through_the_backend() {
        use std::io::{Read, Write};

        let origin = TcpListener::bind("127.0.0.1:0").expect("bind origin");
        let origin_port = origin.local_addr().expect("origin address").port();
        std::thread::spawn(move || {
            for stream in origin.incoming().flatten() {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });

        let root = std::env::temp_dir().join(format!("m3vpn-fast-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::create_dir_all(root.join("core")).unwrap();
        std::fs::create_dir_all(root.join("geo")).unwrap();
        let paths = AppPaths {
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
        };
        let conn = store::open(&paths.db_path).expect("open test database");
        let profile = NewProfile {
            id: "p1".to_string(),
            name: "Test".to_string(),
            url: String::new(),
            url_hidden: true,
            title: None,
            description: None,
            support_url: None,
            web_page_url: None,
            update_interval_hours: 24,
            userinfo: None,
            created_at: 0,
            updated_at: 0,
        };
        let server = NewServer {
            id: "p1:0".to_string(),
            profile_id: "p1".to_string(),
            name: "Local proxy".to_string(),
            protocol: "vless".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(origin_port),
            params_json: None,
            full_config_json: serde_json::json!({
                "inbounds": [],
                "outbounds": [{ "tag": "proxy", "protocol": "freedom" }],
                "routing": {
                    "rules": [{ "type": "field", "outboundTag": "proxy", "network": "tcp,udp" }]
                }
            })
            .to_string(),
            sort_index: 0,
        };
        store::save_import(&conn, &profile, &[server]).expect("save import");
        let db = Mutex::new(conn);
        let sidecars = Sidecars::discover().expect("sidecars");
        for mode in [PingMode::FastHttp, PingMode::ProxyGet, PingMode::ProxyHead] {
            let options =
                PingOptions::new(mode, &format!("http://127.0.0.1:{origin_port}/"), 3000, 1);
            let results = probe_all(&db, &options, Some(&paths), Some(&sidecars)).expect("probe");
            assert_eq!(results.len(), 1);
            assert!(results[0].ok, "{mode:?}: {:?}", results[0].error);
            assert!(
                results[0].latency_ms.is_some(),
                "{mode:?} returned no latency"
            );
        }

        drop(db);
        let _ = std::fs::remove_dir_all(&root);
    }
}
