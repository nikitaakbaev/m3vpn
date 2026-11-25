use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

pub fn patch(
    config: &mut Value,
    socks_port: u16,
    http_port: u16,
    log_path: &Path,
    log_level: &str,
) -> Result<()> {
    let dns_ips = dns_server_ips(config);
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("Xray config must be a JSON object"))?;

    patch_log(root, log_path, log_level);
    patch_inbounds(root, socks_port, http_port);
    ensure_outbounds(root);
    patch_routing(root, &dns_ips);

    let has_observatory = root.contains_key("observatory") || root.contains_key("burstObservatory");
    let has_balancer = root
        .get("routing")
        .and_then(|routing| routing.get("balancers"))
        .and_then(Value::as_array)
        .is_some_and(|balancers| !balancers.is_empty());
    if (has_observatory || has_balancer) && !root.contains_key("stats") {
        root.insert("stats".into(), json!({}));
    }

    Ok(())
}

pub fn write_config(config: &Value, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(config)?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn patch_log(root: &mut Map<String, Value>, log_path: &Path, log_level: &str) {
    let log = root.entry("log").or_insert_with(|| json!({}));
    if !log.is_object() {
        *log = json!({});
    }
    let log = log.as_object_mut().expect("log object");
    log.insert("loglevel".into(), json!(log_level));
    log.insert("error".into(), json!(log_path.display().to_string()));
    log.insert("access".into(), json!(log_path.display().to_string()));
}

fn patch_inbounds(root: &mut Map<String, Value>, socks_port: u16, http_port: u16) {
    let mut inbounds = root.remove("inbounds").unwrap_or_else(|| json!([]));
    if !inbounds.is_array() {
        inbounds = json!([]);
    }
    let inbounds = inbounds.as_array_mut().expect("inbounds array");

    let mut has_socks = false;
    let mut has_http = false;

    for inbound in inbounds.iter_mut() {
        let protocol = inbound
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match protocol.as_str() {
            "socks" | "mixed" => {
                inbound["listen"] = json!("127.0.0.1");
                inbound["port"] = json!(socks_port);
                has_socks = true;
            }
            "http" => {
                inbound["listen"] = json!("127.0.0.1");
                inbound["port"] = json!(http_port);
                has_http = true;
            }
            _ => {}
        }
    }

    if !has_socks {
        inbounds.push(json!({
            "tag": "m3vpn-socks",
            "protocol": "socks",
            "listen": "127.0.0.1",
            "port": socks_port,
            "settings": { "auth": "noauth", "udp": true },
            "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
        }));
    }
    if !has_http {
        inbounds.push(json!({
            "tag": "m3vpn-http",
            "protocol": "http",
            "listen": "127.0.0.1",
            "port": http_port,
            "settings": {}
        }));
    }

    root.insert("inbounds".into(), json!(inbounds));
}

fn ensure_outbounds(root: &mut Map<String, Value>) {
    let outbounds = root.entry("outbounds").or_insert_with(|| json!([]));
    if !outbounds.is_array() {
        *outbounds = json!([]);
    }
    let outbounds = outbounds.as_array_mut().expect("outbounds array");
    let has_direct_tag = outbounds
        .iter()
        .any(|outbound| outbound.get("tag").and_then(Value::as_str) == Some("direct"));
    if !has_direct_tag {
        // patch_routing points DNS probes at the "direct" tag, so the tag must
        // exist even when a differently-named freedom outbound is present.
        outbounds.push(json!({ "tag": "direct", "protocol": "freedom" }));
    }
}

fn patch_routing(root: &mut Map<String, Value>, dns_ips: &[String]) {
    let routing = root
        .entry("routing")
        .or_insert_with(|| json!({ "rules": [] }));
    if !routing.is_object() {
        *routing = json!({ "rules": [] });
    }
    let routing = routing.as_object_mut().expect("routing object");
    let rules = routing.entry("rules").or_insert_with(|| json!([]));
    if !rules.is_array() {
        *rules = json!([]);
    }
    if !dns_ips.is_empty() {
        rules.as_array_mut().expect("rules array").insert(
            0,
            json!({
                "type": "field",
                "ip": dns_ips,
                "outboundTag": "direct"
            }),
        );
    }
}

fn dns_server_ips(config: &Value) -> Vec<String> {
    let mut result = Vec::new();
    let Some(servers) = config.pointer("/dns/servers").and_then(Value::as_array) else {
        return result;
    };
    for server in servers {
        let raw = match server {
            Value::String(value) => Some(value.as_str()),
            Value::Object(map) => map
                .get("address")
                .and_then(Value::as_str)
                .or_else(|| map.get("server").and_then(Value::as_str)),
            _ => None,
        };
        if let Some(raw) = raw {
            if let Some(ip) = extract_ip(raw) {
                if !result.contains(&ip.to_string()) {
                    result.push(ip.to_string());
                }
            }
        }
    }
    result
}

fn extract_ip(raw: &str) -> Option<&str> {
    let rest = raw.split("://").nth(1).unwrap_or(raw);
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host
        .rsplit_once('@')
        .map(|(_, value)| value)
        .unwrap_or(host);
    let host = if host.starts_with('[') {
        host.split(']')
            .next()
            .unwrap_or(host)
            .trim_start_matches('[')
    } else {
        host.split(':').next().unwrap_or(host)
    };
    host.parse::<std::net::IpAddr>().ok().map(|_| host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> Value {
        json!({
            "inbounds": [{ "tag": "socks", "protocol": "socks", "listen": "0.0.0.0", "port": 10808 }],
            "outbounds": [
                { "tag": "proxy", "protocol": "vless", "settings": {} },
                { "tag": "direct", "protocol": "freedom" }
            ],
            "dns": { "servers": ["https://1.1.1.1/dns-query", "8.8.8.8"] },
            "routing": { "rules": [], "balancers": [{ "tag": "b", "selector": ["proxy"] }] },
            "burstObservatory": { "subjectSelector": ["proxy"] }
        })
    }

    #[test]
    fn does_not_mark_xray_sockets() {
        let mut config = json!({
            "inbounds": [],
            "outbounds": [
                { "tag": "proxy", "protocol": "vless", "settings": {} },
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "block", "protocol": "blackhole" }
            ]
        });
        patch(
            &mut config,
            20808,
            20809,
            Path::new("/tmp/xray.log"),
            "warning",
        )
        .unwrap();
        assert!(config["outbounds"][0]
            .get("streamSettings")
            .and_then(|stream| stream.get("sockopt"))
            .is_none());
        assert!(config["outbounds"][1]
            .get("streamSettings")
            .and_then(|stream| stream.get("sockopt"))
            .is_none());
    }

    #[test]
    fn forces_local_inbounds_and_stats() {
        let mut config = base_config();
        patch(
            &mut config,
            20808,
            20809,
            std::path::Path::new("/tmp/xray.log"),
            "warning",
        )
        .unwrap();
        let inbounds = config["inbounds"].as_array().unwrap();
        assert!(inbounds
            .iter()
            .all(|inbound| inbound["listen"] == "127.0.0.1"));
        assert_eq!(inbounds[0]["port"], 20808);
        assert!(config.get("stats").is_some());
    }

    #[test]
    fn adds_direct_dns_rule() {
        let mut config = base_config();
        patch(
            &mut config,
            20808,
            20809,
            std::path::Path::new("/tmp/xray.log"),
            "warning",
        )
        .unwrap();
        let rule = &config["routing"]["rules"][0];
        assert_eq!(rule["outboundTag"], "direct");
        let ips = rule["ip"].as_array().unwrap();
        assert!(ips.iter().any(|ip| ip == "1.1.1.1"));
        assert!(ips.iter().any(|ip| ip == "8.8.8.8"));
    }
}
