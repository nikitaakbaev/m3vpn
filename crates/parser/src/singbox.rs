use crate::ParsedServer;
use serde_json::{json, Value};
use std::collections::HashSet;

/// Protocols that are executed by `hiddify-sing-box` instead of Xray.
const PROTOCOLS: &[&str] = &["awg", "hysteria2", "tuic"];

pub fn uses_singbox(protocol: &str) -> bool {
    PROTOCOLS.contains(&protocol)
}

/// Builds a single-server sing-box config around one proxy outbound.
pub fn wrap(name: String, protocol: &str, host: &str, port: u16, outbound: Value) -> ParsedServer {
    wrap_outbounds(name, protocol, host, port, vec![outbound])
}

/// Builds a sing-box config for one or more proxy outbounds. Multiple
/// outbounds get a `urltest` group named `proxy`, replacing Xray balancers.
pub fn wrap_outbounds(
    name: String,
    protocol: &str,
    host: &str,
    port: u16,
    mut outbounds: Vec<Value>,
) -> ParsedServer {
    if outbounds.len() > 1 {
        let tags = outbounds
            .iter()
            .filter_map(|outbound| outbound.get("tag").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<Vec<_>>();
        outbounds.push(json!({
            "type": "urltest",
            "tag": "proxy",
            "outbounds": tags,
            "url": "https://www.gstatic.com/generate_204",
            "interval": "3m",
            "tolerance": 50
        }));
    }
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));

    let remarks = name.clone();
    let config = json!({
        "log": { "level": "warn", "timestamp": true },
        "inbounds": [{
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": 10808
        }],
        "outbounds": outbounds,
        "route": { "final": "proxy" }
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

/// Converts a full Xray config whose proxy outbounds are Hysteria 2 into a
/// sing-box config. Hysteria 1 cannot be expressed by sing-box's hysteria2
/// outbound, and mixed Xray/sing-box protocol configs stay unsupported.
pub fn from_xray_hysteria(item: &Value) -> Option<ParsedServer> {
    let outbounds = item.get("outbounds")?.as_array()?;
    let mut proxies = Vec::new();
    let mut first_endpoint = None;

    for (index, outbound) in outbounds.iter().enumerate() {
        let protocol = outbound.get("protocol").and_then(Value::as_str)?;
        match protocol {
            "hysteria" => {
                let tag = outbound
                    .get("tag")
                    .and_then(Value::as_str)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("proxy-{index}"));
                let converted = convert_hysteria_outbound(outbound, &tag)?;
                if first_endpoint.is_none() {
                    first_endpoint = Some((converted.1.clone(), converted.2));
                }
                proxies.push(converted.0);
            }
            "freedom" | "blackhole" | "dns" | "loopback" => {}
            _ => return None,
        }
    }

    if proxies.is_empty() {
        return None;
    }
    let (host, port) = first_endpoint?;
    let multiple = proxies.len() > 1;
    let mut used = HashSet::new();
    for (index, proxy) in proxies.iter_mut().enumerate() {
        let current = proxy.get("tag").and_then(Value::as_str).unwrap_or_default();
        let tag =
            if current.is_empty() || used.contains(current) || (multiple && current == "proxy") {
                format!("proxy-{index}")
            } else {
                current.to_string()
            };
        used.insert(tag.clone());
        proxy["tag"] = json!(tag);
    }
    if !multiple {
        proxies[0]["tag"] = json!("proxy");
    }

    let remarks = item
        .get("remarks")
        .and_then(Value::as_str)
        .map(str::to_string);
    let name = remarks
        .clone()
        .or_else(|| {
            proxies
                .first()
                .and_then(|outbound| outbound.get("tag"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Hysteria2 {host}"));

    let mut server = wrap_outbounds(name, "hysteria2", &host, port, proxies);
    server.remarks = remarks;
    Some(server)
}

fn convert_hysteria_outbound(outbound: &Value, tag: &str) -> Option<(Value, String, u16)> {
    let settings = outbound.get("settings")?;
    let host = settings.get("address").and_then(Value::as_str)?.to_string();
    let port = settings.get("port").and_then(Value::as_u64)?;
    if port == 0 || port > u16::MAX as u64 {
        return None;
    }
    let port = port as u16;
    let stream = outbound.get("streamSettings");
    let hysteria = stream.and_then(|stream| stream.get("hysteriaSettings"));

    let version = hysteria
        .and_then(|settings| settings.get("version"))
        .or_else(|| settings.get("version"))
        .and_then(Value::as_u64)
        .unwrap_or(2);
    if version != 2 {
        return None;
    }

    let password = hysteria
        .and_then(|settings| settings.get("auth"))
        .or_else(|| settings.get("auth"))
        .or_else(|| settings.get("password"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let tls_settings = stream.and_then(|stream| stream.get("tlsSettings"));
    let tls = tls_object(tls_settings, &host);

    let mut outbound = json!({
        "type": "hysteria2",
        "tag": tag,
        "server": host,
        "server_port": port,
        "password": password,
        "tls": tls
    });
    if let Some(obfs) = hysteria
        .and_then(|settings| settings.get("obfs"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        let mut object = json!({ "type": obfs });
        if let Some(password) = hysteria
            .and_then(|settings| settings.get("obfsPassword"))
            .and_then(Value::as_str)
        {
            object["password"] = json!(password);
        }
        outbound["obfs"] = object;
    }
    Some((outbound, host, port))
}

fn tls_object(settings: Option<&Value>, host: &str) -> Value {
    let mut tls = json!({ "enabled": true });
    let Some(settings) = settings else {
        tls["server_name"] = json!(host);
        return tls;
    };
    tls["server_name"] = json!(settings
        .get("serverName")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(host));
    if let Some(alpn) = settings.get("alpn").and_then(Value::as_array) {
        tls["alpn"] = json!(alpn);
    }
    // uTLS is not available for QUIC protocols (Hysteria 2/TUIC), so the
    // Xray `fingerprint` setting is intentionally not copied here.
    if insecure(settings.get("allowInsecure")) {
        tls["insecure"] = json!(true);
    }
    tls
}

fn insecure(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        _ => false,
    }
}
