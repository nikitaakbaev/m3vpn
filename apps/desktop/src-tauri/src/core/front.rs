use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

pub const SECRET: &str = "m3vpn";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficStats {
    pub upload_total: u64,
    pub download_total: u64,
    pub connections: usize,
}

pub fn config(socks_port: u16, http_port: u16, api_port: u16, slot_a: u16, slot_b: u16) -> Value {
    json!({
        "log": { "level": "warn", "timestamp": true },
        "inbounds": [
            {
                "type": "mixed",
                "tag": "front-mixed",
                "listen": "127.0.0.1",
                "listen_port": socks_port
            },
            {
                "type": "http",
                "tag": "front-http",
                "listen": "127.0.0.1",
                "listen_port": http_port
            }
        ],
        "outbounds": [
            {
                "type": "selector",
                "tag": "proxy",
                "outbounds": ["upstream-a", "upstream-b", "direct"],
                "default": "direct",
                "interrupt_exist_connections": false
            },
            {
                "type": "socks",
                "tag": "upstream-a",
                "server": "127.0.0.1",
                "server_port": slot_a
            },
            {
                "type": "socks",
                "tag": "upstream-b",
                "server": "127.0.0.1",
                "server_port": slot_b
            },
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": [{ "action": "sniff" }],
            "final": "proxy"
        },
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{api_port}"),
                "secret": SECRET
            }
        }
    })
}

pub fn write_config(config: &Value, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn traffic(api_port: u16) -> Result<TrafficStats> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .context("build traffic client")?;
    let response = client
        .get(format!("http://127.0.0.1:{api_port}/connections"))
        .bearer_auth(SECRET)
        .send()
        .context("fetch front traffic")?;
    if !response.status().is_success() {
        bail!("front traffic returned {}", response.status());
    }
    let value: Value = response.json().context("decode front traffic")?;
    Ok(TrafficStats {
        upload_total: value["uploadTotal"].as_u64().unwrap_or(0),
        download_total: value["downloadTotal"].as_u64().unwrap_or(0),
        connections: value["connections"].as_array().map(Vec::len).unwrap_or(0),
    })
}

pub fn switch(api_port: u16, target: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .context("build front proxy client")?;
    let response = client
        .put(format!("http://127.0.0.1:{api_port}/proxies/proxy"))
        .bearer_auth(SECRET)
        .json(&json!({ "name": target }))
        .send()
        .context("switch front proxy")?;
    if !response.status().is_success() {
        bail!("front proxy switch returned {}", response.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_config_has_no_socket_mark() {
        // The helper marks the proxy cgroup with nftables; the unprivileged
        // sidecar must not try SO_MARK itself (it would fail with EPERM).
        let config = config(10808, 10809, 11308, 11808, 11818);
        assert!(config["route"].get("default_mark").is_none());
    }
}
