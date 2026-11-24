use crate::store::{self, ServerRow};
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    timeout_ms: u64,
    concurrency: usize,
) -> Result<Vec<LatencyResult>> {
    let servers = {
        let db = db.lock().map_err(|_| anyhow!("database lock poisoned"))?;
        store::list_servers(&db, None)?
            .into_iter()
            .filter(|server| server.enabled)
            .collect::<Vec<_>>()
    };

    let timeout = Duration::from_millis(timeout_ms.clamp(500, 10_000));
    let concurrency = concurrency.clamp(1, 16).min(servers.len().max(1));
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(servers.len()));

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(server) = servers.get(index) else {
                    break;
                };
                let result = probe_one(server, timeout);
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

fn probe_one(server: &ServerRow, timeout: Duration) -> LatencyResult {
    let Some(host) = server.host.as_deref() else {
        return LatencyResult {
            server_id: server.id.clone(),
            latency_ms: None,
            ok: false,
            error: Some("нет адреса".to_string()),
        };
    };
    let Some(port) = server.port else {
        return LatencyResult {
            server_id: server.id.clone(),
            latency_ms: None,
            ok: false,
            error: Some("нет порта".to_string()),
        };
    };

    let addresses = match (host, port).to_socket_addrs() {
        Ok(addresses) => addresses.collect::<Vec<_>>(),
        Err(error) => {
            return LatencyResult {
                server_id: server.id.clone(),
                latency_ms: None,
                ok: false,
                error: Some(format!("DNS: {error}")),
            };
        }
    };

    if addresses.is_empty() {
        return LatencyResult {
            server_id: server.id.clone(),
            latency_ms: None,
            ok: false,
            error: Some("DNS вернул пустой список".to_string()),
        };
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

    LatencyResult {
        server_id: server.id.clone(),
        latency_ms: None,
        ok: false,
        error: last_error,
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

    #[test]
    fn probes_a_local_port_and_persists_latency() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("listener address").port();
        std::thread::spawn(move || for _ in listener.incoming() {});

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
        let db = Mutex::new(conn);

        let results = probe_all(&db, 1000, 2).expect("probe");
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
}
