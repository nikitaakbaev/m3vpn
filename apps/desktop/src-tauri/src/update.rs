use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifact {
    pub kind: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub configured: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub artifacts: Vec<UpdateArtifact>,
    pub checked_at: i64,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn empty_info() -> UpdateInfo {
    UpdateInfo {
        configured: false,
        current_version: current_version().to_string(),
        latest_version: None,
        available: false,
        notes: None,
        published_at: None,
        artifacts: Vec::new(),
        checked_at: now_secs(),
    }
}

/// Accepts either a full `latest.json` URL or the directory that contains it.
pub fn manifest_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("URL обновлений не задан");
    }
    let url = if raw.ends_with(".json") {
        raw.to_string()
    } else {
        format!("{}/latest.json", raw.trim_end_matches('/'))
    };
    validate_url(&url)?;
    Ok(url)
}

pub fn check(raw_url: &str) -> Result<UpdateInfo> {
    let url = manifest_url(raw_url)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::none())
        .user_agent(format!("m3vpn/{}/Linux", current_version()))
        .build()
        .context("build update client")?;
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("fetch {url}"))?;
    if !response.status().is_success() {
        bail!("сервер обновлений ответил {}", response.status());
    }
    let manifest: Value = response.json().context("decode update manifest")?;
    parse_manifest(&manifest)
}

fn parse_manifest(manifest: &Value) -> Result<UpdateInfo> {
    let latest = manifest
        .get("version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("в манифесте нет version"))?
        .to_string();
    let notes = manifest
        .get("notes")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let published_at = manifest
        .get("publishedAt")
        .or_else(|| manifest.get("released"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut artifacts = Vec::new();
    if let Some(object) = manifest.get("artifacts").and_then(Value::as_object) {
        for (kind, item) in object {
            let url = item
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("artifact {kind}: нет url"))?;
            validate_url(url)?;
            let sha256 = item
                .get("sha256")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
                .filter(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
                .ok_or_else(|| anyhow::anyhow!("artifact {kind}: нет корректного sha256"))?;
            artifacts.push(UpdateArtifact {
                kind: kind.clone(),
                url: url.to_string(),
                sha256,
                size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            });
        }
    }

    Ok(UpdateInfo {
        configured: true,
        current_version: current_version().to_string(),
        available: is_newer(&latest, current_version()),
        latest_version: Some(latest),
        notes,
        published_at,
        artifacts,
        checked_at: now_secs(),
    })
}

fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    let length = latest.len().max(current.len());
    for index in 0..length {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .take(4)
        .collect()
}

fn validate_url(url: &str) -> Result<()> {
    if url.starts_with("https://") {
        return Ok(());
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let authority = rest.split('/').next().unwrap_or_default();
        let host = authority.rsplit('@').next().unwrap_or_default();
        let host = host.split(':').next().unwrap_or_default();
        if host == "127.0.0.1" || host == "localhost" {
            return Ok(());
        }
    }
    bail!("обновления должны раздаваться по HTTPS")
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

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn parses_signed_manifest() {
        let manifest = serde_json::json!({
            "version": "0.2.0",
            "publishedAt": "2026-12-01",
            "notes": "TUN fixes",
            "artifacts": {
                "arch": {
                    "url": "https://updates.example.com/m3vpn/m3vpn-0.2.0-1-x86_64.pkg.tar.zst",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "size": 123
                }
            }
        });
        let info = parse_manifest(&manifest).expect("manifest");
        assert!(info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.2.0"));
        assert_eq!(info.artifacts.len(), 1);
        assert_eq!(info.artifacts[0].kind, "arch");
    }

    #[test]
    fn downloads_and_parses_a_manifest() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind update server");
        let port = listener.local_addr().expect("address").port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let body = r#"{"version":"0.2.0","notes":"new","artifacts":{"arch":{"url":"https://example.com/m3vpn.pkg.tar.zst","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":10}}}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let info = check(&format!("http://127.0.0.1:{port}")).expect("manifest");
        assert!(info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn rejects_plain_http_manifest() {
        assert!(manifest_url("http://updates.example.com/m3vpn").is_err());
        assert!(manifest_url("http://127.0.0.1:8080/m3vpn").is_ok());
        assert!(manifest_url("http://localhost:8080/m3vpn").is_ok());
        assert!(manifest_url("http://localhost.evil.example/m3vpn").is_err());
        assert!(manifest_url("http://127.0.0.1.evil.example/m3vpn").is_err());
    }
}
