//! Subscription parsing for m3vpn.
//!
//! Supports complete Xray configs (single object or array), share links, and
//! AmneziaWG/Hysteria 2/TUIC servers that run on hiddify-sing-box.

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

mod awg;
mod links;
mod singbox;

pub use singbox::uses_singbox;

const PROXY_PROTOCOLS: &[&str] = &["vless", "vmess", "trojan", "shadowsocks", "socks", "http"];
const XRAY_PROTOCOLS: &[&str] = &[
    "vless",
    "vmess",
    "trojan",
    "shadowsocks",
    "socks",
    "http",
    "wireguard",
    "freedom",
    "blackhole",
    "dns",
    "loopback",
    "hysteria",
];
const UNSUPPORTED_PROXY_PROTOCOLS: &[&str] = &["hysteria", "tuic"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedServer {
    pub name: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub remarks: Option<String>,
    pub config: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub servers: Vec<ParsedServer>,
    pub skipped: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("subscription body is empty")]
    Empty,
    #[error("invalid subscription JSON: {0}")]
    InvalidJson(String),
    #[error("subscription contains no supported servers")]
    NoServers,
    #[error("invalid AmneziaWG config: {0}")]
    InvalidAwg(String),
}

pub fn parse_body(body: &str) -> Result<ParseResult, ParseError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut result = ParseResult::default();

    if looks_like_awg(trimmed) {
        push_awg(trimmed, None, &mut result)?;
    } else if looks_like_json(trimmed) {
        parse_json_body(trimmed, &mut result)?;
    } else if let Some(decoded) = try_base64(trimmed) {
        let decoded = decoded.trim();
        if looks_like_awg(decoded) {
            push_awg(decoded, None, &mut result)?;
        } else if looks_like_json(decoded) {
            parse_json_body(decoded, &mut result)?;
        } else {
            parse_lines(decoded, &mut result);
        }
    } else {
        parse_lines(trimmed, &mut result);
    }

    if result.servers.is_empty() {
        return Err(ParseError::NoServers);
    }
    Ok(result)
}

fn looks_like_json(value: &str) -> bool {
    value.starts_with('{') || value.starts_with('[')
}

fn looks_like_awg(value: &str) -> bool {
    value.contains("[Interface]") && value.to_ascii_lowercase().contains("privatekey")
}

fn push_awg(text: &str, name: Option<String>, result: &mut ParseResult) -> Result<(), ParseError> {
    let mut server = awg::parse_conf(text).map_err(ParseError::InvalidAwg)?;
    if let Some(name) = name.filter(|name| !name.is_empty()) {
        server.name = name;
    }
    result.servers.push(server);
    Ok(())
}

fn try_base64(value: &str) -> Option<String> {
    let compact: String = value.split_whitespace().collect();
    if compact.len() < 16 {
        return None;
    }
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(compact.as_bytes()) {
            if let Ok(text) = String::from_utf8(bytes) {
                let text = text.trim().to_string();
                if text.contains("://") || looks_like_json(&text) || looks_like_awg(&text) {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn parse_json_body(body: &str, result: &mut ParseResult) -> Result<(), ParseError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| ParseError::InvalidJson(error.to_string()))?;
    match value {
        Value::Array(items) => {
            for item in items {
                parse_item(&item, result);
            }
        }
        Value::Object(_) => parse_item(&value, result),
        _ => return Err(ParseError::InvalidJson("expected object or array".into())),
    }
    Ok(())
}

fn parse_item(item: &Value, result: &mut ParseResult) {
    if item.get("inbounds").is_some() && item.get("outbounds").is_some() {
        match full_config_server(item) {
            Some(server) => result.servers.push(server),
            None => {
                result.skipped += 1;
                let hysteria_v1 =
                    item.get("outbounds")
                        .and_then(Value::as_array)
                        .is_some_and(|outbounds| {
                            outbounds.iter().any(|outbound| {
                                if outbound.get("protocol").and_then(Value::as_str)
                                    != Some("hysteria")
                                {
                                    return false;
                                }
                                outbound
                                    .pointer("/streamSettings/hysteriaSettings/version")
                                    .or_else(|| outbound.pointer("/settings/version"))
                                    .and_then(Value::as_u64)
                                    == Some(1)
                            })
                        });
                let note = "hysteria: поддерживается только версия 2";
                if hysteria_v1 && !result.notes.iter().any(|item| item == note) {
                    result.notes.push(note.to_string());
                }
            }
        }
        return;
    }

    if item.get("type").and_then(Value::as_str) == Some("amneziawg") {
        let Some(servers) = item.get("servers").and_then(Value::as_array) else {
            result.skipped += 1;
            return;
        };
        for entry in servers {
            let Some(payload) = entry.get("config").and_then(Value::as_str) else {
                result.skipped += 1;
                continue;
            };
            let Some(conf) = awg::decode_conf_payload(payload) else {
                result.skipped += 1;
                continue;
            };
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Err(error) = push_awg(&conf, name, result) {
                result.skipped += 1;
                result.notes.push(error.to_string());
            }
        }
        return;
    }

    result.skipped += 1;
}

fn full_config_server(item: &Value) -> Option<ParsedServer> {
    let outbounds = item.get("outbounds")?.as_array()?;

    // Provider Hysteria configs use Xray-shaped outbounds, but Xray cannot
    // run Hysteria 2. Convert the whole config for hiddify-sing-box instead.
    if outbounds
        .iter()
        .any(|outbound| outbound.get("protocol").and_then(Value::as_str) == Some("hysteria"))
    {
        return singbox::from_xray_hysteria(item);
    }

    let mut protocols = Vec::new();
    for outbound in outbounds {
        if let Some(protocol) = outbound.get("protocol").and_then(Value::as_str) {
            protocols.push(protocol.to_string());
        }
    }

    if protocols
        .iter()
        .any(|protocol| UNSUPPORTED_PROXY_PROTOCOLS.contains(&protocol.as_str()))
    {
        return None;
    }

    if protocols
        .iter()
        .any(|protocol| !XRAY_PROTOCOLS.contains(&protocol.as_str()))
    {
        return None;
    }

    if !protocols
        .iter()
        .any(|protocol| PROXY_PROTOCOLS.contains(&protocol.as_str()))
    {
        return None;
    }

    let remarks = item
        .get("remarks")
        .and_then(Value::as_str)
        .map(str::to_string);

    let (host, port, protocol) = first_proxy_endpoint(outbounds)?;
    let name = remarks
        .clone()
        .or_else(|| {
            outbounds
                .iter()
                .find(|outbound| {
                    outbound
                        .get("tag")
                        .and_then(Value::as_str)
                        .is_some_and(|tag| tag.starts_with("proxy"))
                })
                .and_then(|outbound| outbound.get("tag"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Server {protocol}"));

    Some(ParsedServer {
        name,
        protocol,
        host,
        port,
        remarks,
        config: item.clone(),
    })
}

fn first_proxy_endpoint(outbounds: &[Value]) -> Option<(Option<String>, Option<u16>, String)> {
    for outbound in outbounds {
        let Some(protocol) = outbound.get("protocol").and_then(Value::as_str) else {
            continue;
        };
        if !PROXY_PROTOCOLS.contains(&protocol) {
            continue;
        }
        let settings = outbound.get("settings");
        let (address, port) = settings
            .and_then(|settings| settings.get("vnext"))
            .and_then(Value::as_array)
            .and_then(|vnext| vnext.first())
            .map(|first| {
                (
                    first
                        .get("address")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    first
                        .get("port")
                        .and_then(Value::as_u64)
                        .map(|port| port as u16),
                )
            })
            .or_else(|| {
                settings
                    .and_then(|settings| settings.get("servers"))
                    .and_then(Value::as_array)
                    .and_then(|servers| servers.first())
                    .map(|first| {
                        (
                            first
                                .get("address")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            first
                                .get("port")
                                .and_then(Value::as_u64)
                                .map(|port| port as u16),
                        )
                    })
            })
            .or_else(|| {
                settings
                    .and_then(|settings| settings.get("peers"))
                    .and_then(Value::as_array)
                    .and_then(|peers| peers.first())
                    .map(|first| {
                        (
                            first
                                .get("endpoint")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            None,
                        )
                    })
            })
            .unwrap_or((None, None));
        return Some((address, port, protocol.to_string()));
    }
    None
}

fn parse_lines(body: &str, result: &mut ParseResult) {
    let parsed = links::parse_lines(body);
    result.servers.extend(parsed.servers);
    result.skipped += parsed.skipped;
    result.notes.extend(parsed.notes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_config(remarks: &str, protocol: &str) -> Value {
        json!({
            "remarks": remarks,
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "port": 10808, "listen": "127.0.0.1" }
            ],
            "outbounds": [
                {
                    "tag": "proxy",
                    "protocol": protocol,
                    "settings": {
                        "vnext": [
                            { "address": "203.0.113.10", "port": 443, "users": [] }
                        ]
                    }
                },
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "block", "protocol": "blackhole" }
            ]
        })
    }

    #[test]
    fn parses_json_array_of_full_configs() {
        let body = serde_json::to_string(&json!([
            full_config("Germany", "vless"),
            full_config("Poland", "vless")
        ]))
        .unwrap();
        let result = parse_body(&body).unwrap();
        assert_eq!(result.servers.len(), 2);
        assert_eq!(result.servers[0].name, "Germany");
        assert_eq!(result.servers[0].protocol, "vless");
        assert_eq!(result.servers[0].host.as_deref(), Some("203.0.113.10"));
        assert_eq!(result.servers[0].port, Some(443));
    }

    fn hysteria_config(remarks: &str, version: u64) -> Value {
        json!({
            "remarks": remarks,
            "inbounds": [
                { "tag": "socks", "protocol": "socks", "port": 10808, "listen": "127.0.0.1" }
            ],
            "outbounds": [
                {
                    "tag": "proxy-paid-1",
                    "protocol": "hysteria",
                    "settings": { "address": "198.51.100.7", "port": 8443, "version": version },
                    "streamSettings": {
                        "network": "hysteria",
                        "security": "tls",
                        "hysteriaSettings": { "auth": "secret", "version": version },
                        "tlsSettings": {
                            "serverName": "hy.example.com",
                            "alpn": ["h3"],
                            "fingerprint": "chrome"
                        }
                    }
                },
                {
                    "tag": "proxy-paid-2",
                    "protocol": "hysteria",
                    "settings": { "address": "198.51.100.8", "port": 9443, "version": version },
                    "streamSettings": {
                        "network": "hysteria",
                        "security": "tls",
                        "hysteriaSettings": { "auth": "secret2", "version": version },
                        "tlsSettings": { "serverName": "hy2.example.com", "alpn": ["h3"] }
                    }
                },
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "block", "protocol": "blackhole" }
            ]
        })
    }

    #[test]
    fn converts_hysteria2_full_config_to_singbox() {
        let body = serde_json::to_string(&json!([
            hysteria_config("Hysteria DE", 2),
            full_config("Reality", "vless")
        ]))
        .unwrap();
        let result = parse_body(&body).unwrap();
        assert_eq!(result.servers.len(), 2);
        let server = result
            .servers
            .iter()
            .find(|server| server.protocol == "hysteria2")
            .unwrap();
        assert_eq!(server.name, "Hysteria DE");
        assert_eq!(server.port, Some(8443));
        assert!(uses_singbox(&server.protocol));
        assert!(server.config.get("remarks").is_none());
        let outbounds = server.config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 4);
        assert_eq!(outbounds[0]["type"], "hysteria2");
        assert_eq!(outbounds[0]["password"], "secret");
        assert_eq!(outbounds[0]["tls"]["server_name"], "hy.example.com");
        assert_eq!(outbounds[1]["server"], "198.51.100.8");
        assert_eq!(outbounds[2]["type"], "urltest");
        assert_eq!(outbounds[2]["tag"], "proxy");
        assert_eq!(
            outbounds[2]["outbounds"],
            json!(["proxy-paid-1", "proxy-paid-2"])
        );
        assert_eq!(outbounds[3]["tag"], "direct");
    }

    #[test]
    fn skips_hysteria_v1_full_configs() {
        let body = serde_json::to_string(&json!([
            hysteria_config("Hysteria v1", 1),
            full_config("Reality", "vless")
        ]))
        .unwrap();
        let result = parse_body(&body).unwrap();
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.skipped, 1);
        assert!(result.notes.iter().any(|note| note.contains("версия 2")));
    }

    #[test]
    fn parses_base64_encoded_json() {
        let inner = serde_json::to_string(&json!([full_config("Base64", "vless")])).unwrap();
        let body = URL_SAFE_NO_PAD.encode(inner.as_bytes());
        let result = parse_body(&body).unwrap();
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].name, "Base64");
    }

    #[test]
    fn routes_singbox_protocols_to_singbox() {
        assert!(uses_singbox("awg"));
        assert!(uses_singbox("hysteria2"));
        assert!(uses_singbox("tuic"));
        assert!(!uses_singbox("vless"));
        assert!(!uses_singbox("trojan"));
    }

    #[test]
    fn empty_body_is_an_error() {
        assert!(matches!(parse_body("   "), Err(ParseError::Empty)));
    }

    #[test]
    fn parses_raw_awg_body() {
        let body = "# Name = Raw AWG\n[Interface]\nPrivateKey = AAAA\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = BBBB\nEndpoint = example.com:51820\nAllowedIPs = 0.0.0.0/0\n";
        let result = parse_body(body).unwrap();
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].protocol, "awg");
        assert_eq!(result.servers[0].name, "Raw AWG");
    }

    #[test]
    fn parses_plain_share_links() {
        let body = "vless://00000000-0000-4000-8000-000000000001@example.com:443?security=tls#Test\ntrojan://pass@example.com:443#Test2";
        let result = parse_body(body).unwrap();
        assert_eq!(result.servers.len(), 2);
        assert_eq!(result.servers[0].protocol, "vless");
        assert_eq!(result.servers[1].protocol, "trojan");
    }

    #[test]
    fn parses_base64_share_links() {
        let body = STANDARD.encode(
            b"vless://00000000-0000-4000-8000-000000000001@example.com:443?security=tls#Base64 link",
        );
        let result = parse_body(&body).unwrap();
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].name, "Base64 link");
    }
}
