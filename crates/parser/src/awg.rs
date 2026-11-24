use crate::ParsedServer;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

const BOOL_TRUE: &[&str] = &["1", "true", "yes", "on", "enabled"];

type IniSection = HashMap<String, String>;
type ParsedIni = (IniSection, IniSection, Vec<IniSection>);

pub fn parse_conf(text: &str) -> Result<ParsedServer, String> {
    let (interface, device, peers) = parse_ini(text);
    let interface = merge_device(interface, device);
    let peer = peers
        .first()
        .ok_or_else(|| "awg: нет секции [Peer]".to_string())?;

    let private_key = interface
        .get("privatekey")
        .ok_or_else(|| "awg: нет PrivateKey".to_string())?
        .clone();
    let endpoint = peer
        .get("endpoint")
        .ok_or_else(|| "awg: нет Endpoint".to_string())?;
    let (host, port) = split_endpoint(endpoint)?;
    let public_key = peer
        .get("publickey")
        .ok_or_else(|| "awg: нет PublicKey".to_string())?
        .clone();

    let name = name_from(text).unwrap_or_else(|| format!("AWG {host}"));
    let mut awg = serde_json::Map::new();
    for key in ["jc", "jmin", "jmax", "s1", "s2", "s3", "s4"] {
        if let Some(value) = interface.get(key) {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| format!("awg: {key} должно быть числом"))?;
            awg.insert(key.to_string(), json!(parsed));
        }
    }
    for key in ["h1", "h2", "h3", "h4", "i1", "i2", "i3", "i4", "i5"] {
        if let Some(value) = interface.get(key) {
            awg.insert(key.to_string(), json!(value));
        }
    }
    for (source, target) in [
        ("contentpaddingaddition", "content_padding_addition"),
        ("rekeyaftertime", "rekey_after_time"),
        ("rekeytimeout", "rekey_timeout"),
        ("rejectaftertime", "reject_after_time"),
        ("keepalivetimeout", "keepalive_timeout"),
        ("maxhandshakeattempts", "max_handshake_attempts"),
    ] {
        if let Some(value) = interface.get(source) {
            awg.insert(target.to_string(), json!(value));
        }
    }
    if let Some(value) = interface.get("headerprotectionkey") {
        awg.insert(
            "header_protection_key".to_string(),
            json!(normalize_hex_key(value)?),
        );
    }
    for (source, target) in [
        ("randomtrailers", "random_trailers"),
        ("disablecookies", "disable_cookies"),
    ] {
        if let Some(value) = interface.get(source) {
            awg.insert(
                target.to_string(),
                json!(BOOL_TRUE.contains(&value.to_ascii_lowercase().as_str())),
            );
        }
    }

    let mut peer_json = json!({
        "address": host,
        "port": port,
        "public_key": public_key,
        "allowed_ips": allowed_ips(peer),
    });
    if let Some(psk) = peer.get("presharedkey") {
        peer_json["preshared_key"] = json!(psk);
    }
    if let Some(keepalive) = peer.get("persistentkeepalive") {
        // AmneziaWG 3.x allows a range; sing-box expects a number, so use the
        // lower bound instead of sending an unsupported string.
        let value = keepalive.split('-').next().unwrap_or(keepalive);
        peer_json["persistent_keepalive_interval"] = json!(value
            .parse::<u64>()
            .map_err(|_| "awg: PersistentKeepalive должно быть числом".to_string())?);
    }

    let tag = "awg-1";
    let endpoint_json = json!({
        "type": "awg",
        "tag": tag,
        "private_key": private_key,
        "address": address_list(&interface),
        "mtu": interface
            .get("mtu")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1408),
        "awg": Value::Object(awg),
        "peers": [peer_json]
    });

    let config = json!({
        "log": { "level": "warn", "timestamp": true },
        "inbounds": [{
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": 10808
        }],
        "endpoints": [endpoint_json],
        "outbounds": [
            {
                "type": "selector",
                "tag": "proxy",
                "outbounds": [tag],
                "default": tag,
                "interrupt_exist_connections": false
            },
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": [{ "action": "sniff" }],
            "final": "proxy"
        },
        "experimental": {
            "clash_api": {
                "external_controller": "127.0.0.1:19090",
                "secret": "m3vpn"
            }
        }
    });

    Ok(ParsedServer {
        name,
        protocol: "awg".to_string(),
        host: Some(host),
        port: Some(port),
        remarks: None,
        config,
    })
}

pub fn decode_conf_payload(payload: &str) -> Option<String> {
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(payload.trim().as_bytes()) {
            if let Ok(text) = String::from_utf8(bytes) {
                if text.contains("[Interface]") {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn parse_ini(text: &str) -> ParsedIni {
    let mut interface = HashMap::new();
    let mut device = HashMap::new();
    let mut peers = Vec::new();
    let mut section = "";
    let mut current_peer: Option<HashMap<String, String>> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            if section.eq_ignore_ascii_case("peer") {
                if let Some(peer) = current_peer.take() {
                    peers.push(peer);
                }
                current_peer = Some(HashMap::new());
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        match section.to_ascii_lowercase().as_str() {
            "interface" => {
                interface.insert(key, value);
            }
            "device" => {
                device.insert(key, value);
            }
            "peer" => {
                if let Some(peer) = current_peer.as_mut() {
                    peer.insert(key, value);
                }
            }
            _ => {}
        }
    }
    if let Some(peer) = current_peer {
        peers.push(peer);
    }
    (interface, device, peers)
}

fn merge_device(mut interface: IniSection, device: IniSection) -> IniSection {
    for (key, value) in device {
        interface.entry(key).or_insert(value);
    }
    interface
}

fn address_list(interface: &HashMap<String, String>) -> Value {
    interface
        .get("address")
        .map(|value| {
            json!(value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>())
        })
        .unwrap_or_else(|| json!(["10.0.0.2/32"]))
}

fn allowed_ips(peer: &HashMap<String, String>) -> Value {
    peer.get("allowedips")
        .map(|value| {
            json!(value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>())
        })
        .unwrap_or_else(|| json!(["0.0.0.0/0", "::/0"]))
}

fn split_endpoint(value: &str) -> Result<(String, u16), String> {
    if value.starts_with('[') {
        let (host, rest) = value
            .trim_start_matches('[')
            .split_once(']')
            .ok_or_else(|| "awg: bad IPv6 endpoint".to_string())?;
        let port = rest
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
            .ok_or_else(|| "awg: bad endpoint port".to_string())?;
        return Ok((host.to_string(), port));
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| "awg: bad endpoint".to_string())?;
    Ok((
        host.to_string(),
        port.parse::<u16>()
            .map_err(|_| "awg: bad endpoint port".to_string())?,
    ))
}

fn name_from(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix('#')?.trim();
        let (key, value) = rest.split_once('=')?;
        if key.trim().eq_ignore_ascii_case("name") {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn normalize_hex_key(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Ok(value.to_ascii_lowercase());
    }
    let bytes = decode_payload(value).ok_or_else(|| "awg: bad HeaderProtectionKey".to_string())?;
    if bytes.len() != 32 {
        return Err("awg: HeaderProtectionKey should be 32 bytes".to_string());
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn decode_payload(value: &str) -> Option<Vec<u8>> {
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(value.as_bytes()) {
            return Some(bytes);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# Name = Germany AWG
[Interface]
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Address = 10.8.0.2/32
MTU = 1408
Jc = 4
Jmin = 40
Jmax = 70
S1 = 86
S2 = 574
H1 = 1234567890
H2 = 1234567891
H3 = 1234567892
H4 = 1234567893

[Peer]
PublicKey = BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=
PresharedKey = CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC=
Endpoint = vpn.example.com:51820
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
"#;

    #[test]
    fn parses_awg_conf_into_sing_box_endpoint() {
        let server = parse_conf(SAMPLE).expect("parse awg");
        assert_eq!(server.protocol, "awg");
        assert_eq!(server.name, "Germany AWG");
        assert_eq!(server.host.as_deref(), Some("vpn.example.com"));
        assert_eq!(server.port, Some(51820));
        let endpoint = &server.config["endpoints"][0];
        assert_eq!(endpoint["type"], "awg");
        assert_eq!(endpoint["awg"]["jc"], 4);
        assert_eq!(endpoint["awg"]["h1"], "1234567890");
        assert_eq!(endpoint["peers"][0]["allowed_ips"][0], "0.0.0.0/0");
    }

    #[test]
    fn rejects_conf_without_peer() {
        let conf = "[Interface]\nPrivateKey = abc\n";
        assert!(parse_conf(conf).is_err());
    }
}
