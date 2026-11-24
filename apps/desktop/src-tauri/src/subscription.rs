use crate::store::{self, NewProfile, NewServer};
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use m3vpn_parser::{parse_body, ParseResult};
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub profile_id: String,
    pub name: String,
    pub title: Option<String>,
    pub servers: usize,
    pub skipped: usize,
    pub notes: Vec<String>,
}

pub fn import(db: &Mutex<Connection>, url: &str, name: Option<String>) -> Result<ImportSummary> {
    import_into(db, url, name, None)
}

pub fn import_into(
    db: &Mutex<Connection>,
    url: &str,
    name: Option<String>,
    existing_id: Option<String>,
) -> Result<ImportSummary> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        bail!("URL подписки должен начинаться с http:// или https://");
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(40))
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build HTTP client")?;

    let response = client
        .get(url)
        .header("Accept", "*/*")
        .header("x-client", "m3vpn")
        .send()
        .context("fetch subscription")?;

    if !response.status().is_success() {
        bail!("сервер подписки ответил {}", response.status());
    }

    let headers = response.headers().clone();
    let body = response.text().context("read subscription body")?;

    let title_header = header(&headers, "profile-title");
    let title = title_header
        .as_deref()
        .and_then(decode_header_value)
        .filter(|value| !value.is_empty());
    let inline_name = header(&headers, "subscription-name")
        .as_deref()
        .and_then(decode_header_value)
        .filter(|value| !value.is_empty());
    let description = header(&headers, "profile-description")
        .as_deref()
        .and_then(decode_header_value);
    let support_url = header(&headers, "support-url");
    let web_page_url =
        header(&headers, "profile-web-page-url").or_else(|| header(&headers, "homepage"));
    let userinfo = header(&headers, "subscription-userinfo");
    let hide_url = header(&headers, "hide-url")
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let update_interval_hours = header(&headers, "profile-update-interval")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (1..=24 * 30).contains(value))
        .unwrap_or(24);

    let parsed = parse_body(&body).map_err(|error| anyhow!(error.to_string()))?;
    let profile_name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| title.clone())
        .or(inline_name)
        .unwrap_or_else(|| "Подписка".to_string());

    save_parsed(
        db,
        parsed,
        ProfileMeta {
            name: profile_name,
            url: url.to_string(),
            title,
            description,
            support_url,
            web_page_url,
            update_interval_hours,
            userinfo,
            hide_url,
            source: "subscription",
            existing_id,
        },
    )
}

pub fn import_awg(
    db: &Mutex<Connection>,
    conf: &str,
    name: Option<String>,
) -> Result<ImportSummary> {
    let parsed = parse_body(conf.trim()).map_err(|error| anyhow!(error.to_string()))?;
    if !parsed.servers.iter().any(|server| server.protocol == "awg") {
        bail!("в тексте не найден AmneziaWG .conf");
    }
    let profile_name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| parsed.servers.first().map(|server| server.name.clone()))
        .unwrap_or_else(|| "AmneziaWG".to_string());

    save_parsed(
        db,
        parsed,
        ProfileMeta {
            name: profile_name,
            url: String::new(),
            title: Some("AmneziaWG".to_string()),
            description: None,
            support_url: None,
            web_page_url: None,
            update_interval_hours: 24,
            userinfo: None,
            hide_url: true,
            source: "amneziawg",
            existing_id: None,
        },
    )
}

struct ProfileMeta {
    name: String,
    url: String,
    title: Option<String>,
    description: Option<String>,
    support_url: Option<String>,
    web_page_url: Option<String>,
    update_interval_hours: u32,
    userinfo: Option<String>,
    hide_url: bool,
    source: &'static str,
    existing_id: Option<String>,
}

fn save_parsed(
    db: &Mutex<Connection>,
    parsed: ParseResult,
    meta: ProfileMeta,
) -> Result<ImportSummary> {
    let profile_name = meta.name.chars().take(80).collect::<String>();
    let now = now_secs();
    let profile_id = meta
        .existing_id
        .unwrap_or_else(|| format!("p{}", now_millis()));
    let mut servers = Vec::with_capacity(parsed.servers.len());

    for (index, server) in parsed.servers.iter().enumerate() {
        let params = serde_json::json!({
            "remarks": server.remarks,
            "protocol": server.protocol,
            "source": meta.source,
        });
        servers.push(NewServer {
            id: format!("{profile_id}:{index}"),
            profile_id: profile_id.clone(),
            name: server.name.chars().take(120).collect(),
            protocol: server.protocol.clone(),
            host: server.host.clone(),
            port: server.port,
            params_json: Some(params.to_string()),
            full_config_json: serde_json::to_string(&server.config)?,
            sort_index: index as i64,
        });
    }

    let profile = NewProfile {
        id: profile_id.clone(),
        name: profile_name.clone(),
        url: meta.url,
        url_hidden: meta.hide_url,
        title: meta.title.clone(),
        description: meta.description,
        support_url: meta.support_url,
        web_page_url: meta.web_page_url,
        update_interval_hours: meta.update_interval_hours,
        userinfo: meta.userinfo,
        created_at: now,
        updated_at: now,
    };

    {
        let db = db.lock().map_err(|_| anyhow!("database lock poisoned"))?;
        store::save_import(&db, &profile, &servers)?;
    }

    tracing::info!(
        profile = %profile_name,
        source = meta.source,
        servers = servers.len(),
        skipped = parsed.skipped,
        "subscription imported"
    );

    Ok(ImportSummary {
        profile_id,
        name: profile_name,
        title: meta.title,
        servers: servers.len(),
        skipped: parsed.skipped,
        notes: parsed.notes,
    })
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn decode_header_value(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(encoded) = value.strip_prefix("base64:") {
        let bytes = STANDARD.decode(encoded.trim()).ok()?;
        return String::from_utf8(bytes)
            .ok()
            .map(|text| text.trim().to_string());
    }
    Some(value.to_string())
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn serve_once(response: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let port = listener.local_addr().expect("test address").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}/sub")
    }

    #[test]
    fn imports_full_config_and_headers() {
        let body = serde_json::to_string(&serde_json::json!([{
            "remarks": "Test DE",
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "listen": "127.0.0.1", "port": 10808 }
            ],
            "outbounds": [
                {
                    "tag": "proxy",
                    "protocol": "vless",
                    "settings": {
                        "vnext": [{ "address": "203.0.113.10", "port": 443, "users": [] }]
                    }
                },
                { "tag": "direct", "protocol": "freedom" }
            ]
        }]))
        .expect("serialize fixture");

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nprofile-title: Test profile\r\nprofile-update-interval: 6\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let url = serve_once(response);

        let path = std::env::temp_dir().join(format!("m3vpn-sub-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = store::open(&path).expect("open test database");
        let db = Mutex::new(conn);

        let summary = import(&db, &url, None).expect("import subscription");
        assert_eq!(summary.servers, 1);
        assert_eq!(summary.name, "Test profile");

        let profiles = store::list_profiles(&db.lock().unwrap()).expect("profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].server_count, 1);
        assert_eq!(profiles[0].update_interval_hours, 6);

        let servers = store::list_servers(&db.lock().unwrap(), None).expect("servers");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Test DE");
        assert_eq!(servers[0].host.as_deref(), Some("203.0.113.10"));
        assert_eq!(servers[0].port, Some(443));

        drop(db);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }

    #[test]
    fn refresh_updates_profile_in_place() {
        fn body(remarks: &str) -> String {
            serde_json::to_string(&serde_json::json!([{
                "remarks": remarks,
                "inbounds": [
                    { "tag": "socks", "protocol": "socks", "listen": "127.0.0.1", "port": 10808 }
                ],
                "outbounds": [
                    {
                        "tag": "proxy",
                        "protocol": "vless",
                        "settings": { "vnext": [{ "address": "203.0.113.10", "port": 443, "users": [] }] }
                    },
                    { "tag": "direct", "protocol": "freedom" }
                ]
            }]))
            .unwrap()
        }

        let response = |body: &str| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
        };

        let first_url = serve_once(response(&body("First")));
        let second_url = serve_once(response(&body("Second")));

        let path =
            std::env::temp_dir().join(format!("m3vpn-refresh-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = store::open(&path).expect("open test database");
        let db = Mutex::new(conn);

        let first = import(&db, &first_url, Some("Profile".to_string())).expect("first import");
        let second = import_into(
            &db,
            &second_url,
            Some("Profile renamed".to_string()),
            Some(first.profile_id.clone()),
        )
        .expect("refresh import");

        assert_eq!(second.profile_id, first.profile_id);
        let profiles = store::list_profiles(&db.lock().unwrap()).expect("profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Profile renamed");

        let servers = store::list_servers(&db.lock().unwrap(), None).expect("servers");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Second");

        drop(db);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }

    #[test]
    #[ignore = "requires the local AmneziaWG server on 127.0.0.1:51830"]
    fn imports_awg_conf_and_connects_with_singbox() {
        use crate::core::{self, CoreState};
        use crate::paths::AppPaths;
        use crate::sidecars::Sidecars;

        let conf = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../.secrets/awg-client.conf"),
        )
        .expect("AWG client conf");

        let root = std::env::temp_dir().join(format!("m3vpn-awg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::create_dir_all(root.join("core")).unwrap();
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
        };

        let conn = store::open(&paths.db_path).expect("open test database");
        let db = Mutex::new(conn);
        let summary = import_awg(&db, &conf, Some("AWG Test".to_string())).expect("import awg");
        assert_eq!(summary.servers, 1);

        let servers = store::list_servers(&db.lock().unwrap(), None).expect("servers");
        let details = store::server_details(&db.lock().unwrap(), &servers[0].id)
            .unwrap()
            .unwrap();
        assert_eq!(details.protocol, "awg");
        let config: serde_json::Value = serde_json::from_str(&details.full_config_json).unwrap();

        let sidecars = Sidecars::discover().expect("sidecars");
        let state = Mutex::new(CoreState::new(22808));
        let status = core::connect_singbox(
            &state,
            &paths,
            &sidecars,
            &servers[0].id,
            &details.name,
            config,
        )
        .expect("connect sing-box");
        assert!(status.running);
        assert_eq!(status.engine.as_deref(), Some("sing-box"));

        let output = std::process::Command::new("curl")
            .arg("-fsS")
            .arg("--max-time")
            .arg("20")
            .arg("--socks5-hostname")
            .arg("127.0.0.1:22808")
            .arg("https://www.cloudflare.com/cdn-cgi/trace")
            .output()
            .expect("run curl through sing-box");
        assert!(
            output.status.success(),
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("loc="));

        let status = core::shutdown(&state, &paths);
        assert!(!status.running);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "hits the real subscription and starts xray"]
    fn imports_real_subscription_and_connects() {
        use crate::core::{self, CoreState};
        use crate::paths::AppPaths;
        use crate::sidecars::Sidecars;

        let url = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../.secrets/subscription.url"),
        )
        .expect("subscription URL file")
        .trim()
        .to_string();

        let root = std::env::temp_dir().join(format!("m3vpn-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::create_dir_all(root.join("core")).unwrap();

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
        };

        let conn = store::open(&paths.db_path).expect("open test database");
        let db = Mutex::new(conn);
        let summary = import(&db, &url, Some("E2E".to_string())).expect("import real subscription");
        assert!(summary.servers > 0, "no servers imported");

        let servers = store::list_servers(&db.lock().unwrap(), None).expect("list servers");
        let sidecars = Sidecars::discover().expect("sidecars");
        let state = Mutex::new(CoreState::new(22808));

        let mut connected = false;
        for server in servers.iter().take(4) {
            let details = store::server_details(&db.lock().unwrap(), &server.id)
                .unwrap()
                .unwrap();
            let config: serde_json::Value =
                serde_json::from_str(&details.full_config_json).unwrap();
            match core::connect_xray(&state, &paths, &sidecars, &server.id, &details.name, config) {
                Ok(status) => {
                    eprintln!("connected to {} ({})", server.name, status.socks_port);
                    let output = std::process::Command::new("curl")
                        .arg("-fsS")
                        .arg("--max-time")
                        .arg("25")
                        .arg("--socks5-hostname")
                        .arg(format!("127.0.0.1:{}", status.socks_port))
                        .arg("https://www.cloudflare.com/cdn-cgi/trace")
                        .output()
                        .expect("run curl through xray");
                    if output.status.success() {
                        let text = String::from_utf8_lossy(&output.stdout);
                        eprintln!("{text}");
                        assert!(text.contains("loc="), "unexpected trace output");
                        connected = true;
                        break;
                    }
                    eprintln!(
                        "curl failed for {}: {}",
                        server.name,
                        String::from_utf8_lossy(&output.stderr)
                    );
                    core::disconnect(&state, &paths);
                }
                Err(error) => {
                    eprintln!("connect failed for {}: {error:#}", server.name);
                }
            }
        }

        assert!(connected, "no imported server accepted a connection");
        let status = core::shutdown(&state, &paths);
        assert!(!status.running);
        let _ = std::fs::remove_dir_all(&root);
    }
}
