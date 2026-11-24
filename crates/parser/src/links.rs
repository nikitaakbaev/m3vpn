use crate::ParsedServer;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use percent_encoding::percent_decode_str;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use url::Url;

const UNSUPPORTED_SCHEMES: &[&str] = &["hysteria", "wireguard", "wg", "ssr"];

pub struct LinkResult {
    pub servers: Vec<ParsedServer>,
    pub skipped: usize,
    pub notes: Vec<String>,
}

pub fn parse_lines(body: &str) -> LinkResult {
    let mut servers = Vec::new();
    let mut skipped = 0;
    let mut notes = BTreeSet::new();

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let scheme = line
            .split("://")
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if UNSUPPORTED_SCHEMES.contains(&scheme.as_str()) {
            skipped += 1;
            notes.insert(format!("{scheme}: пока не поддерживается"));
            continue;
        }

        match parse_line(line) {
            Ok(server) => servers.push(server),
            Err(reason) => {
                skipped += 1;
                notes.insert(reason);
            }
        }
    }

    LinkResult {
        servers,
        skipped,
        notes: notes.into_iter().collect(),
    }
}

fn parse_line(line: &str) -> Result<ParsedServer, String> {
    if line.starts_with("vless://") {
        parse_vless(line)
    } else if line.starts_with("vmess://") {
        parse_vmess(line)
    } else if line.starts_with("trojan://") {
        parse_trojan(line)
    } else if line.starts_with("ss://") {
        parse_ss(line)
    } else if line.starts_with("socks://") || line.starts_with("socks5://") {
        parse_socks(line)
    } else if line.starts_with("http://") || line.starts_with("https://") {
        parse_http(line)
    } else if line.starts_with("hysteria2://") || line.starts_with("hy2://") {
        parse_hysteria2(line)
    } else if line.starts_with("tuic://") {
        parse_tuic(line)
    } else if line.starts_with("amneziawg://") || line.starts_with("awg://") {
        parse_awg_link(line)
    } else {
        Err(format!(
            "{}: неизвестная схема",
            line.split("://").next().unwrap_or("link")
        ))
    }
}

fn parse_vless(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("vless: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "vless: нет хоста".to_string())?
        .to_string();
    let port = url.port().unwrap_or(443);
    let uuid = decode(url.username());
    if uuid.is_empty() {
        return Err("vless: пустой UUID".to_string());
    }
    if param(&url, "security").as_deref() == Some("reality")
        && param(&url, "pbk").unwrap_or_default().is_empty()
    {
        return Err("vless: reality без publicKey".to_string());
    }
    let flow = param(&url, "flow").unwrap_or_default();
    let name = name_from(&url, &format!("VLESS {host}"));
    let outbound = json!({
        "tag": "proxy",
        "protocol": "vless",
        "settings": {
            "vnext": [{
                "address": host,
                "port": port,
                "users": [{ "id": uuid, "encryption": "none", "flow": flow }]
            }]
        },
        "streamSettings": stream_settings(&url, &host, false)
    });
    Ok(wrap(name, "vless", &host, port, outbound))
}

fn parse_trojan(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("trojan: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "trojan: нет хоста".to_string())?
        .to_string();
    let port = url.port().unwrap_or(443);
    let password = decode(url.username());
    if password.is_empty() {
        return Err("trojan: пустой пароль".to_string());
    }
    let name = name_from(&url, &format!("Trojan {host}"));
    let outbound = json!({
        "tag": "proxy",
        "protocol": "trojan",
        "settings": {
            "servers": [{ "address": host, "port": port, "password": password }]
        },
        "streamSettings": stream_settings(&url, &host, true)
    });
    Ok(wrap(name, "trojan", &host, port, outbound))
}

fn parse_vmess(line: &str) -> Result<ParsedServer, String> {
    let encoded = line
        .strip_prefix("vmess://")
        .ok_or_else(|| "vmess: bad prefix".to_string())?;
    let decoded = decode_base64(encoded).ok_or_else(|| "vmess: bad base64".to_string())?;
    let data: Value =
        serde_json::from_str(&decoded).map_err(|error| format!("vmess: bad json: {error}"))?;

    let host = data["add"]
        .as_str()
        .ok_or_else(|| "vmess: no address".to_string())?
        .to_string();
    let port = data["port"]
        .as_str()
        .and_then(|value| value.parse::<u16>().ok())
        .or_else(|| data["port"].as_u64().map(|value| value as u16))
        .ok_or_else(|| "vmess: no port".to_string())?;
    let id = data["id"]
        .as_str()
        .ok_or_else(|| "vmess: no id".to_string())?
        .to_string();
    let alter_id = data["aid"]
        .as_u64()
        .or_else(|| data["alterId"].as_u64())
        .unwrap_or(0) as u32;
    let security = data["scy"]
        .as_str()
        .or_else(|| data["security"].as_str())
        .unwrap_or("auto");
    let network = data["net"].as_str().unwrap_or("tcp");
    let tls = data["tls"].as_str().unwrap_or("none");
    let sni = data["sni"]
        .as_str()
        .or_else(|| data["host"].as_str())
        .unwrap_or(&host);
    let name = data["ps"]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("VMess {host}"));

    let mut stream = json!({
        "network": network,
        "security": if tls == "tls" { "tls" } else { "none" }
    });
    if tls == "tls" {
        stream["tlsSettings"] = json!({ "serverName": sni });
    }
    match network {
        "ws" => {
            stream["wsSettings"] = json!({
                "path": data["path"].as_str().unwrap_or("/"),
                "headers": { "Host": data["host"].as_str().unwrap_or(&host) }
            });
        }
        "grpc" => {
            stream["grpcSettings"] = json!({
                "serviceName": data["path"].as_str().unwrap_or("")
            });
        }
        "h2" | "http" => {
            stream["httpSettings"] = json!({
                "path": data["path"].as_str().unwrap_or("/"),
                "host": [data["host"].as_str().unwrap_or(&host)]
            });
        }
        _ => {}
    }

    let outbound = json!({
        "tag": "proxy",
        "protocol": "vmess",
        "settings": {
            "vnext": [{
                "address": host,
                "port": port,
                "users": [{
                    "id": id,
                    "alterId": alter_id,
                    "security": security,
                    "level": 0
                }]
            }]
        },
        "streamSettings": stream
    });
    Ok(wrap(name, "vmess", &host, port, outbound))
}

fn parse_ss(line: &str) -> Result<ParsedServer, String> {
    let rest = line
        .strip_prefix("ss://")
        .ok_or_else(|| "ss: bad prefix".to_string())?;
    let (body, name) = split_fragment(rest);
    let body = body.split('?').next().unwrap_or(body);

    let (method, password, host, port) = if let Some((userinfo, hostport)) = body.rsplit_once('@') {
        let decoded = decode_base64(userinfo).unwrap_or_else(|| decode(userinfo));
        let (method, password) = decoded
            .split_once(':')
            .ok_or_else(|| "ss: bad userinfo".to_string())?;
        let (host, port) = split_host_port(hostport)?;
        (method.to_string(), password.to_string(), host, port)
    } else {
        let decoded = decode_base64(body).ok_or_else(|| "ss: bad base64".to_string())?;
        let (userinfo, hostport) = decoded
            .rsplit_once('@')
            .ok_or_else(|| "ss: no host".to_string())?;
        let (method, password) = userinfo
            .split_once(':')
            .ok_or_else(|| "ss: bad userinfo".to_string())?;
        let (host, port) = split_host_port(hostport)?;
        (method.to_string(), password.to_string(), host, port)
    };

    let name = name.unwrap_or_else(|| format!("SS {host}"));
    let outbound = json!({
        "tag": "proxy",
        "protocol": "shadowsocks",
        "settings": {
            "servers": [{
                "address": host,
                "port": port,
                "method": method,
                "password": password
            }]
        }
    });
    Ok(wrap(name, "shadowsocks", &host, port, outbound))
}

fn parse_socks(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("socks: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "socks: нет хоста".to_string())?
        .to_string();
    let port = url.port().unwrap_or(1080);
    let user = decode(url.username());
    let pass = url.password().map(decode).unwrap_or_default();
    let name = name_from(&url, &format!("SOCKS5 {host}"));

    let mut server = json!({ "address": host, "port": port });
    if !user.is_empty() {
        server["users"] = json!([{ "user": user, "pass": pass }]);
    }
    let outbound = json!({
        "tag": "proxy",
        "protocol": "socks",
        "settings": { "servers": [server] }
    });
    Ok(wrap(name, "socks", &host, port, outbound))
}

fn parse_hysteria2(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("hysteria2: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "hysteria2: нет хоста".to_string())?
        .to_string();
    let port = url.port().unwrap_or(443);
    let mut password = decode(url.username());
    if let Some(tail) = url.password() {
        if !password.is_empty() {
            password.push(':');
        }
        password.push_str(&decode(tail));
    }
    if password.is_empty() {
        return Err("hysteria2: пустой пароль".to_string());
    }
    let name = name_from(&url, &format!("Hysteria2 {host}"));

    let mut outbound = json!({
        "type": "hysteria2",
        "tag": "proxy",
        "server": host,
        "server_port": port,
        "password": password,
        "tls": quic_tls(&url, &host)
    });
    if let Some(obfs) = param(&url, "obfs").filter(|value| !value.is_empty()) {
        outbound["obfs"] = json!({
            "type": obfs,
            "password": param(&url, "obfs-password").unwrap_or_default()
        });
    }
    Ok(crate::singbox::wrap(
        name,
        "hysteria2",
        &host,
        port,
        outbound,
    ))
}

fn parse_tuic(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("tuic: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "tuic: нет хоста".to_string())?
        .to_string();
    let port = url.port().unwrap_or(443);
    let uuid = decode(url.username());
    let password = url.password().map(decode).unwrap_or_default();
    if uuid.is_empty() {
        return Err("tuic: пустой UUID".to_string());
    }
    if password.is_empty() {
        return Err("tuic: пустой пароль".to_string());
    }
    let name = name_from(&url, &format!("TUIC {host}"));

    let mut outbound = json!({
        "type": "tuic",
        "tag": "proxy",
        "server": host,
        "server_port": port,
        "uuid": uuid,
        "password": password,
        "tls": quic_tls(&url, &host)
    });
    if let Some(control) = param(&url, "congestion_control") {
        outbound["congestion_control"] = json!(control);
    }
    if let Some(mode) = param(&url, "udp_relay_mode") {
        outbound["udp_relay_mode"] = json!(mode);
    }
    if bool_param(&url, "udp_over_stream") {
        outbound["udp_over_stream"] = json!(true);
    }
    if bool_param(&url, "zero_rtt_handshake") {
        outbound["zero_rtt_handshake"] = json!(true);
    }
    Ok(crate::singbox::wrap(name, "tuic", &host, port, outbound))
}

fn quic_tls(url: &Url, host: &str) -> Value {
    let mut tls = json!({
        "enabled": true,
        "server_name": param(url, "sni")
            .or_else(|| param(url, "peer"))
            .unwrap_or_else(|| host.to_string())
    });
    let alpn = param(url, "alpn")
        .map(|value| {
            value
                .split(',')
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| vec!["h3".to_string()]);
    tls["alpn"] = json!(alpn);
    if bool_param(url, "insecure") || bool_param(url, "allow_insecure") {
        tls["insecure"] = json!(true);
    }
    // QUIC protocols in sing-box reject uTLS, so `fingerprint` is ignored.
    tls
}

fn bool_param(url: &Url, key: &str) -> bool {
    param(url, key)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn parse_awg_link(line: &str) -> Result<ParsedServer, String> {
    let rest = line
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or_else(|| "awg: bad link".to_string())?;
    let (payload, name) = split_fragment(rest);
    let conf = crate::awg::decode_conf_payload(payload)
        .ok_or_else(|| "awg: bad base64 conf".to_string())?;
    let mut server = crate::awg::parse_conf(&conf)?;
    if let Some(name) = name {
        server.name = name;
    }
    Ok(server)
}

fn parse_http(line: &str) -> Result<ParsedServer, String> {
    let url = Url::parse(line).map_err(|error| format!("http: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "http: нет хоста".to_string())?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(8080);
    let user = decode(url.username());
    let pass = url.password().map(decode).unwrap_or_default();
    let name = name_from(&url, &format!("HTTP {host}"));

    let mut server = json!({ "address": host, "port": port });
    if !user.is_empty() {
        server["users"] = json!([{ "user": user, "pass": pass }]);
    }
    let mut outbound = json!({
        "tag": "proxy",
        "protocol": "http",
        "settings": { "servers": [server] }
    });
    if url.scheme() == "https" {
        outbound["streamSettings"] = json!({
            "network": "tcp",
            "security": "tls",
            "tlsSettings": { "serverName": &host }
        });
    }
    Ok(wrap(name, "http", &host, port, outbound))
}

fn wrap(name: String, protocol: &str, host: &str, port: u16, outbound: Value) -> ParsedServer {
    let remarks = name.clone();
    let config = json!({
        "remarks": remarks,
        "log": { "loglevel": "warning" },
        "inbounds": [{
            "tag": "socks",
            "protocol": "socks",
            "listen": "127.0.0.1",
            "port": 10808,
            "settings": { "auth": "noauth", "udp": true },
            "sniffing": { "enabled": true, "destOverride": ["http", "tls"] }
        }],
        "outbounds": [
            outbound,
            { "tag": "direct", "protocol": "freedom" },
            { "tag": "block", "protocol": "blackhole" }
        ],
        "routing": {
            "rules": [{ "type": "field", "outboundTag": "proxy", "network": "tcp,udp" }]
        }
    });
    ParsedServer {
        name,
        protocol: protocol.to_string(),
        host: Some(host.to_string()),
        port: Some(port),
        remarks: Some(remarks),
        config,
    }
}

fn stream_settings(url: &Url, host: &str, default_tls: bool) -> Value {
    let network = param(url, "type").unwrap_or_else(|| "tcp".to_string());
    let security = param(url, "security").unwrap_or_else(|| {
        if default_tls {
            "tls".to_string()
        } else {
            "none".to_string()
        }
    });
    let mut stream = json!({ "network": network.clone(), "security": security });

    match security.as_str() {
        "tls" => {
            let sni = param(url, "sni")
                .or_else(|| param(url, "host"))
                .unwrap_or_else(|| host.to_string());
            let mut tls = json!({ "serverName": sni });
            if let Some(alpn) = param(url, "alpn") {
                tls["alpn"] = json!(alpn
                    .split(',')
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>());
            }
            if let Some(fingerprint) = param(url, "fp") {
                tls["fingerprint"] = json!(fingerprint);
            }
            if param(url, "allowInsecure").as_deref() == Some("1") {
                tls["allowInsecure"] = json!(true);
            }
            stream["tlsSettings"] = tls;
        }
        "reality" => {
            stream["realitySettings"] = json!({
                "serverName": param(url, "sni").unwrap_or_else(|| host.to_string()),
                "fingerprint": param(url, "fp").unwrap_or_else(|| "chrome".to_string()),
                "publicKey": param(url, "pbk").unwrap_or_default(),
                "shortId": param(url, "sid").unwrap_or_default(),
                "spiderX": param(url, "spx").unwrap_or_else(|| "/".to_string())
            });
        }
        _ => {}
    }

    match network.as_str() {
        "ws" => {
            stream["wsSettings"] = json!({
                "path": param(url, "path").unwrap_or_else(|| "/".to_string()),
                "headers": { "Host": param(url, "host").unwrap_or_else(|| host.to_string()) }
            });
        }
        "grpc" => {
            stream["grpcSettings"] = json!({
                "serviceName": param(url, "serviceName").unwrap_or_default()
            });
        }
        "http" | "h2" => {
            stream["httpSettings"] = json!({
                "path": param(url, "path").unwrap_or_else(|| "/".to_string()),
                "host": [param(url, "host").unwrap_or_else(|| host.to_string())]
            });
        }
        "xhttp" => {
            stream["xhttpSettings"] = json!({
                "path": param(url, "path").unwrap_or_else(|| "/".to_string()),
                "host": param(url, "host").unwrap_or_else(|| host.to_string())
            });
        }
        "tcp" if param(url, "headerType").as_deref() == Some("http") => {
            stream["tcpSettings"] = json!({
                "header": {
                    "type": "http",
                    "request": {
                        "path": [param(url, "path").unwrap_or_else(|| "/".to_string())],
                        "headers": {
                            "Host": [param(url, "host").unwrap_or_else(|| host.to_string())]
                        }
                    }
                }
            });
        }
        _ => {}
    }

    stream
}

fn split_fragment(value: &str) -> (&str, Option<String>) {
    match value.rsplit_once('#') {
        Some((body, fragment)) => (body, Some(decode(fragment))),
        None => (value, None),
    }
}

fn split_host_port(value: &str) -> Result<(String, u16), String> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| "bad IPv6 host".to_string())?;
        let port = tail
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
            .ok_or_else(|| "bad port".to_string())?;
        return Ok((host.to_string(), port));
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| "bad host:port".to_string())?;
    let port = port.parse::<u16>().map_err(|_| "bad port".to_string())?;
    Ok((host.to_string(), port))
}

fn name_from(url: &Url, fallback: &str) -> String {
    url.fragment()
        .map(decode)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

fn decode(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().to_string()
}

fn decode_base64(value: &str) -> Option<String> {
    let compact: String = value.split_whitespace().collect();
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(compact.as_bytes()) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vless_reality() {
        let line = "vless://00000000-0000-4000-8000-000000000001@example.com:443?security=reality&type=tcp&sni=cdn.example.com&fp=chrome&pbk=PUBKEY&sid=abcd#Germany";
        let result = parse_lines(line);
        assert_eq!(result.servers.len(), 1);
        let server = &result.servers[0];
        assert_eq!(server.protocol, "vless");
        assert_eq!(server.name, "Germany");
        assert_eq!(server.host.as_deref(), Some("example.com"));
        assert_eq!(server.port, Some(443));
        assert_eq!(
            server.config["outbounds"][0]["streamSettings"]["security"],
            "reality"
        );
        assert_eq!(
            server.config["outbounds"][0]["streamSettings"]["realitySettings"]["publicKey"],
            "PUBKEY"
        );
    }

    #[test]
    fn parses_trojan_and_ss() {
        let body = [
            "trojan://secret@example.com:443?sni=example.com#Trojan",
            "ss://YWVzLTI1Ni1nY206cGFzcw==@example.com:8388#SS",
        ]
        .join("\n");
        let result = parse_lines(&body);
        assert_eq!(result.servers.len(), 2);
        assert_eq!(result.servers[0].protocol, "trojan");
        assert_eq!(result.servers[1].protocol, "shadowsocks");
        assert_eq!(result.servers[1].name, "SS");
    }

    #[test]
    fn parses_vmess_json() {
        let payload = json!({
            "v": "2",
            "ps": "VMess DE",
            "add": "example.com",
            "port": "443",
            "id": "00000000-0000-4000-8000-000000000002",
            "aid": "0",
            "net": "ws",
            "path": "/ws",
            "host": "example.com",
            "tls": "tls"
        });
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let result = parse_lines(&format!("vmess://{encoded}"));
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].name, "VMess DE");
        assert_eq!(result.servers[0].protocol, "vmess");
        assert_eq!(
            result.servers[0].config["outbounds"][0]["streamSettings"]["network"],
            "ws"
        );
    }

    #[test]
    fn parses_amneziawg_link() {
        let conf = "# Name = Link AWG\n[Interface]\nPrivateKey = AAAA\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = BBBB\nEndpoint = example.com:51820\nAllowedIPs = 0.0.0.0/0\n";
        let encoded = URL_SAFE_NO_PAD.encode(conf);
        let result = parse_lines(&format!("amneziawg://{encoded}#From link"));
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].protocol, "awg");
        assert_eq!(result.servers[0].name, "From link");
    }

    #[test]
    fn parses_hysteria2_link() {
        let line = "hysteria2://secret@example.com:8443?sni=cdn.example.com&insecure=1&obfs=salamander&obfs-password=obfs&alpn=h3#HY2";
        let result = parse_lines(line);
        assert_eq!(result.servers.len(), 1);
        let server = &result.servers[0];
        assert_eq!(server.protocol, "hysteria2");
        assert_eq!(server.name, "HY2");
        assert_eq!(server.port, Some(8443));
        let outbound = &server.config["outbounds"][0];
        assert_eq!(outbound["type"], "hysteria2");
        assert_eq!(outbound["password"], "secret");
        assert_eq!(outbound["tls"]["server_name"], "cdn.example.com");
        assert_eq!(outbound["tls"]["insecure"], true);
        assert_eq!(outbound["obfs"]["type"], "salamander");
    }

    #[test]
    fn parses_tuic_link() {
        let line = "tuic://00000000-0000-4000-8000-000000000003:pw@example.com:443?congestion_control=bbr&udp_relay_mode=quic&sni=example.com#TUIC";
        let result = parse_lines(line);
        assert_eq!(result.servers.len(), 1);
        let server = &result.servers[0];
        assert_eq!(server.protocol, "tuic");
        let outbound = &server.config["outbounds"][0];
        assert_eq!(outbound["type"], "tuic");
        assert_eq!(outbound["uuid"], "00000000-0000-4000-8000-000000000003");
        assert_eq!(outbound["password"], "pw");
        assert_eq!(outbound["congestion_control"], "bbr");
        assert_eq!(outbound["udp_relay_mode"], "quic");
    }

    #[test]
    fn skips_hysteria_v1() {
        let result = parse_lines("hysteria://pass@example.com:443#HY");
        assert_eq!(result.servers.len(), 0);
        assert_eq!(result.skipped, 1);
        assert!(result.notes.iter().any(|note| note.contains("hysteria")));
    }
}
