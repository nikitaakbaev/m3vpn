use crate::paths::AppPaths;
use crate::sidecars::Sidecars;
use crate::store::{self, Db};
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use reqwest::blocking::Client;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_GEOIP: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat";
const DEFAULT_GEOSITE: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat";
const RU_GEOIP: &str =
    "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/geoip.dat";
const RU_GEOSITE: &str =
    "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/geosite.dat";

#[derive(Clone, Debug, Default)]
pub struct RoutingProfile {
    pub name: String,
    pub global_proxy: bool,
    pub remote_dns_type: String,
    pub remote_dns_domain: String,
    pub remote_dns_ip: String,
    pub domestic_dns_type: String,
    pub domestic_dns_domain: String,
    pub domestic_dns_ip: String,
    pub geoip_url: String,
    pub geosite_url: String,
    pub dns_hosts: HashMap<String, String>,
    pub direct_sites: Vec<String>,
    pub direct_ip: Vec<String>,
    pub proxy_sites: Vec<String>,
    pub proxy_ip: Vec<String>,
    pub block_sites: Vec<String>,
    pub block_ip: Vec<String>,
    pub domain_strategy: String,
    pub fake_dns: bool,
    pub last_updated: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingSummary {
    pub id: String,
    pub name: String,
    pub source_url: Option<String>,
    pub last_updated: Option<i64>,
    pub geo_last_check_at: Option<i64>,
    pub active: bool,
    pub profile_json: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoStatus {
    pub geoip_bytes: u64,
    pub geosite_bytes: u64,
    pub checked_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetStatus {
    pub tag: String,
    pub bytes: u64,
    pub modified_at: Option<i64>,
}

/// Lists the cached sing-box rule-sets so the UI can show which categories
/// are ready for the next connection.
pub fn cached_rule_sets(paths: &AppPaths) -> Vec<RuleSetStatus> {
    let dir = paths.geo_dir.join("rulesets");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("srs") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let (kind, name) = match stem.split_once('-') {
            Some((kind, name)) if kind == "geosite" || kind == "geoip" => (kind, name),
            _ => continue,
        };
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|delta| delta.as_secs() as i64);
        result.push(RuleSetStatus {
            tag: format!("{kind}:{name}"),
            bytes: metadata.len(),
            modified_at,
        });
    }
    result.sort_by(|left, right| left.tag.cmp(&right.tag));
    result
}

pub fn parse_profile(text: &str) -> Result<(RoutingProfile, Value, Option<String>)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        bail!("профиль пустой");
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let body = fetch_text(trimmed)?;
        let (profile, value, _) = parse_profile(&body)?;
        return Ok((profile, value, Some(trimmed.to_string())));
    }
    let payload = extract_payload(trimmed)?;
    let value: Value =
        serde_json::from_str(&payload).map_err(|error| anyhow!("JSON профиля: {error}"))?;
    if !value.is_object() {
        bail!("профиль должен быть JSON-объектом");
    }
    let profile = profile_from_value(&value);
    Ok((profile, value, None))
}

fn extract_payload(text: &str) -> Result<String> {
    if text.starts_with('{') {
        return Ok(text.to_string());
    }
    let mut candidate = text.trim();
    if candidate.contains("://routing/") || candidate.contains("/onadd/") {
        for marker in ["/onadd/", "/add/", "/routing/"] {
            if let Some((_, rest)) = candidate.split_once(marker) {
                candidate = rest.trim();
                if candidate.starts_with('{') {
                    return Ok(candidate.to_string());
                }
                break;
            }
        }
    }
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(candidate.as_bytes()) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                let decoded = decoded.trim().to_string();
                if decoded.starts_with('{') {
                    return Ok(decoded);
                }
            }
        }
    }
    bail!("не удалось декодировать профиль (JSON/base64/deep link)")
}

fn profile_from_value(value: &Value) -> RoutingProfile {
    RoutingProfile {
        name: string(value, &["Name", "name"]).unwrap_or_else(|| "Default".to_string()),
        global_proxy: boolean(value, &["GlobalProxy", "globalProxy"]).unwrap_or(true),
        remote_dns_type: string(value, &["RemoteDNSType", "remoteDnsType"])
            .unwrap_or_else(|| "DoH".to_string()),
        remote_dns_domain: string(value, &["RemoteDNSDomain", "remoteDnsDomain"])
            .unwrap_or_else(|| "https://cloudflare-dns.com/dns-query".to_string()),
        remote_dns_ip: string(value, &["RemoteDNSIP", "RemoteDns", "remoteDnsIp"])
            .unwrap_or_else(|| "1.1.1.1".to_string()),
        domestic_dns_type: string(value, &["DomesticDNSType", "domesticDnsType"])
            .unwrap_or_else(|| "DoU".to_string()),
        domestic_dns_domain: string(value, &["DomesticDNSDomain", "domesticDnsDomain"])
            .unwrap_or_default(),
        domestic_dns_ip: string(value, &["DomesticDNSIP", "DomesticDns", "domesticDnsIp"])
            .unwrap_or_else(|| "8.8.8.8".to_string()),
        geoip_url: string(value, &["Geoipurl", "GeoIPUrl", "geoipUrl"]).unwrap_or_default(),
        geosite_url: string(value, &["Geositeurl", "GeoSiteUrl", "geositeUrl"]).unwrap_or_default(),
        dns_hosts: map(value, &["DnsHosts", "dnsHosts"]),
        direct_sites: list(value, &["DirectSites", "directSites"]),
        direct_ip: list(value, &["DirectIp", "directIp"]),
        proxy_sites: list(value, &["ProxySites", "proxySites"]),
        proxy_ip: list(value, &["ProxyIp", "proxyIp"]),
        block_sites: list(value, &["BlockSites", "blockSites"]),
        block_ip: list(value, &["BlockIp", "blockIp"]),
        domain_strategy: string(value, &["DomainStrategy", "domainStrategy"])
            .unwrap_or_else(|| "AsIs".to_string()),
        fake_dns: boolean(value, &["FakeDNS", "fakeDns"]).unwrap_or(false),
        last_updated: value
            .get("LastUpdated")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok())),
    }
}

fn string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn boolean(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| match value.get(*key) {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::String(value)) => Some(matches!(
            value.to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )),
        _ => None,
    })
}

fn list(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        let Some(item) = value.get(*key) else {
            continue;
        };
        if let Some(array) = item.as_array() {
            return array
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(text) = item.as_str() {
            return text
                .split([',', '\n'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect();
        }
    }
    Vec::new()
}

fn map(value: &Value, keys: &[&str]) -> HashMap<String, String> {
    for key in keys {
        if let Some(object) = value.get(*key).and_then(Value::as_object) {
            return object
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect();
        }
    }
    HashMap::new()
}

pub fn save(
    db: &Db,
    id: Option<&str>,
    profile: &RoutingProfile,
    raw: &Value,
    source_url: Option<String>,
) -> Result<String> {
    let now = now_secs();
    let id = id
        .map(str::to_string)
        .unwrap_or_else(|| format!("r{}", now_millis()));
    let json = serde_json::to_string_pretty(raw)?;
    let geoip = if profile.geoip_url.is_empty() {
        None
    } else {
        Some(profile.geoip_url.clone())
    };
    let geosite = if profile.geosite_url.is_empty() {
        None
    } else {
        Some(profile.geosite_url.clone())
    };

    db.execute(
        "INSERT INTO routing_profiles(
            id, name, source_url, auto_update, profile_json, last_updated,
            geoip_url, geosite_url, geo_last_check_at, geo_next_check_at
         ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, NULL, NULL)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            source_url = excluded.source_url,
            profile_json = excluded.profile_json,
            last_updated = excluded.last_updated,
            geoip_url = excluded.geoip_url,
            geosite_url = excluded.geosite_url",
        params![
            id,
            profile.name,
            source_url,
            json,
            profile.last_updated.unwrap_or(now),
            geoip,
            geosite,
        ],
    )?;
    Ok(id)
}

pub fn list_profiles(db: &Db) -> Result<Vec<RoutingSummary>> {
    let active = store::get(db, "routing.active_profile_id")?.unwrap_or_default();
    let mut stmt = db.prepare(
        "SELECT id, name, source_url, last_updated, geo_last_check_at, profile_json
         FROM routing_profiles ORDER BY last_updated DESC, name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        Ok(RoutingSummary {
            active: id == active,
            id,
            name: row.get(1)?,
            source_url: row.get(2)?,
            last_updated: row.get(3)?,
            geo_last_check_at: row.get(4)?,
            profile_json: row.get(5)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get(db: &Db, id: &str) -> Result<Option<(RoutingProfile, String)>> {
    let record: Option<String> = db
        .query_row(
            "SELECT profile_json FROM routing_profiles WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(json) = record else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(&json)?;
    Ok(Some((profile_from_value(&value), json)))
}

pub fn active(db: &Db) -> Result<Option<(String, RoutingProfile)>> {
    let Some(id) = store::get(db, "routing.active_profile_id")? else {
        return Ok(None);
    };
    if id.is_empty() {
        return Ok(None);
    }
    Ok(get(db, &id)?.map(|(profile, _)| (id, profile)))
}

pub fn set_active(db: &Db, id: &str) -> Result<()> {
    if get(db, id)?.is_none() {
        bail!("профиль маршрутизации не найден");
    }
    store::set(db, "routing.active_profile_id", id)
}

pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.execute("DELETE FROM routing_profiles WHERE id = ?1", [id])?;
    if store::get(db, "routing.active_profile_id")?.as_deref() == Some(id) {
        store::set(db, "routing.active_profile_id", "")?;
    }
    Ok(())
}

pub fn geo_due(db: &Db, id: &str, now: i64) -> Result<bool> {
    let next: Option<Option<i64>> = db
        .query_row(
            "SELECT geo_next_check_at FROM routing_profiles WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(next.flatten().map(|next| next <= now).unwrap_or(true))
}

pub fn record_geo_check(db: &Db, id: &str, checked_at: i64, next: i64) -> Result<()> {
    db.execute(
        "UPDATE routing_profiles SET geo_last_check_at = ?1, geo_next_check_at = ?2 WHERE id = ?3",
        params![checked_at, next, id],
    )?;
    Ok(())
}

pub fn apply_xray(config: &mut Value, profile: &RoutingProfile) -> Result<()> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("Xray config must be an object"))?;
    ensure_outbound(root, "direct", "freedom");
    ensure_outbound(root, "block", "blackhole");

    let (proxy_tag, balancer) = proxy_target(root);
    let mut rules = Vec::new();
    let mut dns_ips = Vec::new();
    for ip in [&profile.remote_dns_ip, &profile.domestic_dns_ip] {
        if ip.parse::<std::net::IpAddr>().is_ok() && !dns_ips.contains(ip) {
            dns_ips.push(ip.clone());
        }
    }
    if !dns_ips.is_empty() {
        rules.push(json!({
            "type": "field",
            "ip": dns_ips,
            "outboundTag": "direct"
        }));
    }
    rules.extend(
        vec![
            rule(
                profile.direct_sites.clone(),
                profile.direct_ip.clone(),
                Target::Direct,
            ),
            rule(
                profile.proxy_sites.clone(),
                profile.proxy_ip.clone(),
                Target::Proxy {
                    tag: proxy_tag.clone(),
                    balancer: balancer.clone(),
                },
            ),
            rule(
                profile.block_sites.clone(),
                profile.block_ip.clone(),
                Target::Block,
            ),
        ]
        .into_iter()
        .flatten(),
    );

    let routing = root
        .entry("routing")
        .or_insert_with(|| json!({ "rules": [] }));
    if !routing.is_object() {
        *routing = json!({ "rules": [] });
    }
    let routing = routing.as_object_mut().expect("routing object");
    routing.insert(
        "domainStrategy".into(),
        json!(match profile.domain_strategy.as_str() {
            "IPIfNonMatch" | "IPOnDemand" => profile.domain_strategy.clone(),
            _ => "AsIs".to_string(),
        }),
    );
    let existing = routing.entry("rules").or_insert_with(|| json!([]));
    if !existing.is_array() {
        *existing = json!([]);
    }
    let existing = existing.as_array_mut().expect("rules array");
    let insert_at = existing
        .iter()
        .rposition(|rule| rule.get("inboundTag").is_some())
        .map(|index| index + 1)
        .unwrap_or(0);
    for (offset, rule) in rules.into_iter().enumerate() {
        existing.insert(insert_at + offset, rule);
    }

    if !profile.global_proxy {
        let flipped = existing.iter_mut().rev().find(|rule| is_catch_all(rule));
        if let Some(rule) = flipped {
            if let Some(object) = rule.as_object_mut() {
                object.remove("balancerTag");
                object.insert("outboundTag".into(), json!("direct"));
            }
        } else {
            existing.push(json!({
                "type": "field",
                "network": "tcp,udp",
                "outboundTag": "direct"
            }));
        }
    }

    apply_dns(root, profile);
    Ok(())
}

fn apply_dns(root: &mut Map<String, Value>, profile: &RoutingProfile) {
    let remote = dns_address(
        &profile.remote_dns_type,
        &profile.remote_dns_domain,
        &profile.remote_dns_ip,
        "https://cloudflare-dns.com/dns-query",
        "1.1.1.1",
    );
    let domestic = dns_address(
        &profile.domestic_dns_type,
        &profile.domestic_dns_domain,
        &profile.domestic_dns_ip,
        "",
        "8.8.8.8",
    );
    if remote.is_empty() && domestic.is_empty() && profile.dns_hosts.is_empty() && !profile.fake_dns
    {
        return;
    }
    if profile.fake_dns {
        root.insert(
            "fakedns".into(),
            json!({ "ipPool": "198.18.0.0/15", "poolSize": 65535 }),
        );
    }
    let mut servers = Vec::new();
    if profile.fake_dns {
        servers.push(json!("fakedns"));
    }
    if !remote.is_empty() {
        servers.push(json!(remote));
    }
    if !domestic.is_empty() {
        servers.push(json!(domestic));
    }
    let dns = root.entry("dns").or_insert_with(|| json!({}));
    if !dns.is_object() {
        *dns = json!({});
    }
    let dns = dns.as_object_mut().expect("dns object");
    if !servers.is_empty() {
        dns.insert("servers".into(), json!(servers));
    }
    if !profile.dns_hosts.is_empty() {
        dns.insert("hosts".into(), json!(profile.dns_hosts));
    }
}

/// DNS servers handed to the helper for TUN mode. Only the remote resolver is
/// used: every TUN query is proxied, so a domestic resolver would leak. The
/// helper adds a proxied bootstrap for resolver hostnames.
pub fn tun_dns_servers(profile: &RoutingProfile) -> Vec<Value> {
    let remote = dns_address(
        &profile.remote_dns_type,
        &profile.remote_dns_domain,
        &profile.remote_dns_ip,
        "https://cloudflare-dns.com/dns-query",
        "1.1.1.1",
    );
    if remote.is_empty() {
        return Vec::new();
    }
    vec![dns_server_from_address(&remote)]
}

fn dns_server_from_address(address: &str) -> Value {
    let lower = address.to_ascii_lowercase();
    let (kind, rest) = if let Some(rest) = lower.strip_prefix("https://") {
        ("https", &address[address.len() - rest.len()..])
    } else if let Some(rest) = lower.strip_prefix("tls://") {
        ("tls", &address[address.len() - rest.len()..])
    } else if let Some(rest) = lower.strip_prefix("quic://") {
        ("quic", &address[address.len() - rest.len()..])
    } else if let Some(rest) = lower.strip_prefix("tcp://") {
        ("tcp", &address[address.len() - rest.len()..])
    } else if let Some(rest) = lower.strip_prefix("udp://") {
        ("udp", &address[address.len() - rest.len()..])
    } else {
        ("udp", address)
    };
    let (authority, path) = if kind == "https" {
        match rest.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (rest, "/dns-query".to_string()),
        }
    } else {
        (rest, String::new())
    };
    let (server, port) = split_host_port(authority);
    let mut value = json!({ "type": kind, "server": server });
    if let Some(port) = port {
        value["server_port"] = json!(port);
    }
    if kind == "https" {
        value["path"] = json!(path);
    }
    value
}

fn split_host_port(authority: &str) -> (&str, Option<u16>) {
    if let Some((host, port)) = authority.rsplit_once(':') {
        if !host.is_empty() && !host.ends_with(':') {
            if let Ok(port) = port.parse::<u16>() {
                return (host, Some(port));
            }
        }
    }
    (authority, None)
}

fn dns_address(kind: &str, domain: &str, ip: &str, fallback: &str, fallback_ip: &str) -> String {
    if kind.eq_ignore_ascii_case("DoH") {
        if domain.starts_with("http://") || domain.starts_with("https://") {
            domain.to_string()
        } else if !domain.is_empty() {
            format!("https://{domain}/dns-query")
        } else if !ip.is_empty() {
            format!("https://{ip}/dns-query")
        } else {
            fallback.to_string()
        }
    } else if !ip.is_empty() {
        ip.to_string()
    } else {
        fallback_ip.to_string()
    }
}

#[derive(Clone)]
enum Target {
    Direct,
    Block,
    Proxy {
        tag: Option<String>,
        balancer: Option<String>,
    },
}

fn rule(sites: Vec<String>, ips: Vec<String>, target: Target) -> Option<Value> {
    if sites.is_empty() && ips.is_empty() {
        return None;
    }
    let mut object = Map::new();
    object.insert("type".into(), json!("field"));
    if !sites.is_empty() {
        object.insert(
            "domain".into(),
            json!(sites.into_iter().map(normalize_site).collect::<Vec<_>>()),
        );
    }
    if !ips.is_empty() {
        object.insert("ip".into(), json!(ips));
    }
    match target {
        Target::Direct => {
            object.insert("outboundTag".into(), json!("direct"));
        }
        Target::Block => {
            object.insert("outboundTag".into(), json!("block"));
        }
        Target::Proxy { tag, balancer } => {
            if let Some(balancer) = balancer {
                object.insert("balancerTag".into(), json!(balancer));
            } else if let Some(tag) = tag {
                object.insert("outboundTag".into(), json!(tag));
            } else {
                object.insert("outboundTag".into(), json!("proxy"));
            }
        }
    }
    Some(Value::Object(object))
}

// Loyalsoldier's geosite.dat has no `RU` entry: Russian rules live under
// `CATEGORY-RU`. Provider profiles commonly use `geosite:ru`, which Xray rejects
// at config load time, so map the shorthand to the bundled category.
fn normalize_site(site: String) -> String {
    if site.eq_ignore_ascii_case("geosite:ru") {
        "geosite:category-ru".to_string()
    } else {
        site
    }
}

fn proxy_target(root: &Map<String, Value>) -> (Option<String>, Option<String>) {
    let rules = root
        .get("routing")
        .and_then(|routing| routing.get("rules"))
        .and_then(Value::as_array);
    if let Some(rules) = rules {
        for rule in rules.iter().rev() {
            if let Some(tag) = rule.get("balancerTag").and_then(Value::as_str) {
                return (None, Some(tag.to_string()));
            }
        }
    }
    let outbounds = root.get("outbounds").and_then(Value::as_array);
    if let Some(outbounds) = outbounds {
        for outbound in outbounds {
            let protocol = outbound
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("");
            if matches!(
                protocol,
                "vless" | "vmess" | "trojan" | "shadowsocks" | "socks" | "http"
            ) {
                return (
                    outbound
                        .get("tag")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    None,
                );
            }
        }
    }
    (Some("proxy".to_string()), None)
}

fn ensure_outbound(root: &mut Map<String, Value>, tag: &str, protocol: &str) {
    let outbounds = root.entry("outbounds").or_insert_with(|| json!([]));
    if !outbounds.is_array() {
        *outbounds = json!([]);
    }
    let outbounds = outbounds.as_array_mut().expect("outbounds");
    if !outbounds
        .iter()
        .any(|outbound| outbound.get("tag").and_then(Value::as_str) == Some(tag))
    {
        outbounds.push(json!({ "tag": tag, "protocol": protocol }));
    }
}

fn is_catch_all(rule: &Value) -> bool {
    if rule.get("type").and_then(Value::as_str) != Some("field") {
        return false;
    }
    for key in [
        "domain",
        "ip",
        "inboundTag",
        "port",
        "sourcePort",
        "processName",
        "protocol",
        "user",
    ] {
        if rule.get(key).is_some() {
            return false;
        }
    }
    rule.get("network")
        .and_then(Value::as_str)
        .map(|network| network.contains("tcp"))
        .unwrap_or(true)
}

#[derive(Clone, Debug)]
pub struct SingboxRuleSet {
    pub tag: String,
    pub path: PathBuf,
}

/// Downloads the sing-box rule-sets referenced by a routing profile. Missing
/// or failed categories are skipped so a profile never prevents connecting.
pub fn ensure_singbox_rule_sets(paths: &AppPaths, profile: &RoutingProfile) -> Vec<SingboxRuleSet> {
    let dir = paths.geo_dir.join("rulesets");
    let _ = std::fs::create_dir_all(&dir);
    let mut wanted = Vec::new();
    for item in profile
        .direct_sites
        .iter()
        .chain(profile.proxy_sites.iter())
        .chain(profile.block_sites.iter())
    {
        if let Some(tag) = item.strip_prefix("geosite:") {
            let tag = normalize_singbox_geo(tag);
            if valid_geo_tag(&tag)
                && !wanted
                    .iter()
                    .any(|(kind, value)| *kind == "geosite" && value == &tag)
            {
                wanted.push(("geosite", tag));
            }
        }
    }
    for item in profile
        .direct_ip
        .iter()
        .chain(profile.proxy_ip.iter())
        .chain(profile.block_ip.iter())
    {
        if let Some(tag) = item.strip_prefix("geoip:") {
            let tag = tag.to_ascii_lowercase();
            if valid_geo_tag(&tag)
                && !wanted
                    .iter()
                    .any(|(kind, value)| *kind == "geoip" && value == &tag)
            {
                wanted.push(("geoip", tag));
            }
        }
    }

    let client = match Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "cannot build rule-set client");
            return Vec::new();
        }
    };
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut result = Vec::new();
    for (kind, tag) in wanted {
        if Instant::now() >= deadline {
            tracing::warn!("rule-set download budget exceeded");
            break;
        }
        let filename = format!("{kind}-{tag}.srs");
        let path = dir.join(&filename);
        if !path.exists() || path.metadata().map(|meta| meta.len() == 0).unwrap_or(true) {
            let url = format!(
                "https://raw.githubusercontent.com/SagerNet/sing-{kind}/rule-set/{filename}"
            );
            let response = match client.get(&url).send() {
                Ok(response) if response.status().is_success() => response,
                Ok(response) => {
                    tracing::warn!(url = %url, status = %response.status(), "rule-set download failed");
                    continue;
                }
                Err(error) => {
                    tracing::warn!(url = %url, %error, "rule-set download failed");
                    continue;
                }
            };
            let bytes = match response.bytes() {
                Ok(bytes) if !bytes.is_empty() => bytes,
                _ => continue,
            };
            let temp = path.with_extension("tmp");
            if std::fs::write(&temp, &bytes).is_err() || std::fs::rename(&temp, &path).is_err() {
                let _ = std::fs::remove_file(&temp);
                continue;
            }
        }
        result.push(SingboxRuleSet {
            tag: format!("m3vpn-{kind}-{tag}"),
            path,
        });
    }
    result
}

fn normalize_singbox_geo(tag: &str) -> String {
    if tag.eq_ignore_ascii_case("ru") {
        "category-ru".to_string()
    } else {
        tag.to_ascii_lowercase()
    }
}

fn valid_geo_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

pub fn apply_singbox(
    config: &mut Value,
    profile: &RoutingProfile,
    rule_sets: &[SingboxRuleSet],
) -> Result<()> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("sing-box config must be an object"))?;
    let route = root
        .entry("route")
        .or_insert_with(|| json!({ "rules": [] }));
    if !route.is_object() {
        *route = json!({ "rules": [] });
    }
    let route = route.as_object_mut().expect("route");
    route.insert(
        "final".into(),
        json!(if profile.global_proxy {
            "proxy"
        } else {
            "direct"
        }),
    );
    let rules = route.entry("rules").or_insert_with(|| json!([]));
    if !rules.is_array() {
        *rules = json!([]);
    }
    let rules = rules.as_array_mut().expect("route rules");
    let mut prepend = Vec::new();
    let mut used_sets: Vec<&SingboxRuleSet> = Vec::new();
    if !profile.direct_sites.is_empty()
        || !profile.proxy_sites.is_empty()
        || !profile.block_sites.is_empty()
        || !profile.direct_ip.is_empty()
        || !profile.proxy_ip.is_empty()
        || !profile.block_ip.is_empty()
    {
        // Domain rules only match after sniffing, so the sniff action must
        // precede the profile rules we prepend to the route.
        prepend.push(json!({ "action": "sniff" }));
    }
    push_singbox_domain_rules(
        &mut prepend,
        &mut used_sets,
        &profile.direct_sites,
        "direct",
        rule_sets,
    );
    push_singbox_ip_rules(
        &mut prepend,
        &mut used_sets,
        &profile.direct_ip,
        "direct",
        rule_sets,
    );
    push_singbox_domain_rules(
        &mut prepend,
        &mut used_sets,
        &profile.proxy_sites,
        "proxy",
        rule_sets,
    );
    push_singbox_ip_rules(
        &mut prepend,
        &mut used_sets,
        &profile.proxy_ip,
        "proxy",
        rule_sets,
    );
    push_singbox_domain_rules(
        &mut prepend,
        &mut used_sets,
        &profile.block_sites,
        "reject",
        rule_sets,
    );
    push_singbox_ip_rules(
        &mut prepend,
        &mut used_sets,
        &profile.block_ip,
        "reject",
        rule_sets,
    );
    for rule in prepend.into_iter().rev() {
        rules.insert(0, rule);
    }
    if !used_sets.is_empty() {
        route.insert(
            "rule_set".into(),
            json!(used_sets
                .iter()
                .map(|set| json!({
                    "type": "local",
                    "tag": set.tag,
                    "path": set.path.display().to_string()
                }))
                .collect::<Vec<_>>()),
        );
    }
    if profile.fake_dns {
        apply_singbox_fakeip(root, profile);
    }
    Ok(())
}

fn push_singbox_domain_rules<'a>(
    rules: &mut Vec<Value>,
    used_sets: &mut Vec<&'a SingboxRuleSet>,
    sites: &[String],
    target: &str,
    rule_sets: &'a [SingboxRuleSet],
) {
    let domains = singbox_domains(sites);
    if !domains.is_empty() {
        rules.push(singbox_target(json!({ "domain_suffix": domains }), target));
    }
    for tag in geosite_tags(sites) {
        if let Some(set) = rule_sets
            .iter()
            .find(|set| set.tag == format!("m3vpn-geosite-{tag}"))
        {
            if !used_sets.iter().any(|item| item.tag == set.tag) {
                used_sets.push(set);
            }
            rules.push(singbox_target(json!({ "rule_set": [set.tag] }), target));
        }
    }
}

fn push_singbox_ip_rules<'a>(
    rules: &mut Vec<Value>,
    used_sets: &mut Vec<&'a SingboxRuleSet>,
    ips: &[String],
    target: &str,
    rule_sets: &'a [SingboxRuleSet],
) {
    let cidrs = singbox_ips(ips);
    if !cidrs.is_empty() {
        rules.push(singbox_target(json!({ "ip_cidr": cidrs }), target));
    }
    for tag in geoip_tags(ips) {
        if let Some(set) = rule_sets
            .iter()
            .find(|set| set.tag == format!("m3vpn-geoip-{tag}"))
        {
            if !used_sets.iter().any(|item| item.tag == set.tag) {
                used_sets.push(set);
            }
            rules.push(singbox_target(json!({ "rule_set": [set.tag] }), target));
        }
    }
}

fn singbox_target(mut rule: Value, target: &str) -> Value {
    if target == "reject" {
        rule["action"] = json!("reject");
    } else {
        rule["outbound"] = json!(target);
    }
    rule
}

fn geosite_tags(items: &[String]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| item.strip_prefix("geosite:"))
        .map(normalize_singbox_geo)
        .filter(|tag| valid_geo_tag(tag))
        .collect()
}

fn geoip_tags(items: &[String]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| item.strip_prefix("geoip:"))
        .map(|tag| tag.to_ascii_lowercase())
        .filter(|tag| valid_geo_tag(tag))
        .collect()
}

fn apply_singbox_fakeip(root: &mut Map<String, Value>, profile: &RoutingProfile) {
    let domains = singbox_domains(&profile.proxy_sites);
    if domains.is_empty() {
        return;
    }
    let dns = root.entry("dns").or_insert_with(|| json!({}));
    if !dns.is_object() {
        *dns = json!({});
    }
    let dns = dns.as_object_mut().expect("dns object");
    let servers = dns.entry("servers").or_insert_with(|| json!([]));
    if !servers.is_array() {
        *servers = json!([]);
    }
    let servers = servers.as_array_mut().expect("dns servers");
    let needs_default = servers.is_empty();
    if needs_default {
        let resolver = [&profile.domestic_dns_ip, &profile.remote_dns_ip]
            .into_iter()
            .find(|ip| ip.parse::<std::net::IpAddr>().is_ok())
            .cloned()
            .unwrap_or_else(|| "1.1.1.1".to_string());
        servers.push(json!({
            "type": "udp",
            "tag": "m3vpn-dns",
            "server": resolver
        }));
    }
    if !servers
        .iter()
        .any(|server| server.get("tag").and_then(Value::as_str) == Some("m3vpn-fakeip"))
    {
        servers.push(json!({
            "type": "fakeip",
            "tag": "m3vpn-fakeip",
            "inet4_range": "198.18.0.0/15",
            "inet6_range": "2001:2::/48"
        }));
    }
    if needs_default {
        dns.insert("final".into(), json!("m3vpn-dns"));
    }
    let rules = dns.entry("rules").or_insert_with(|| json!([]));
    if !rules.is_array() {
        *rules = json!([]);
    }
    let rules = rules.as_array_mut().expect("dns rules");
    rules.insert(
        0,
        json!({
            "domain_suffix": domains,
            "server": "m3vpn-fakeip"
        }),
    );
}

fn singbox_domains(items: &[String]) -> Vec<String> {
    items
        .iter()
        .filter(|item| !item.starts_with("geosite:") && !item.starts_with("regexp:"))
        .map(|item| {
            item.trim_start_matches("full:")
                .trim_start_matches("domain:")
                .to_string()
        })
        .collect()
}

fn singbox_ips(items: &[String]) -> Vec<String> {
    items
        .iter()
        .filter(|item| !item.starts_with("geoip:"))
        .cloned()
        .collect()
}

pub fn ensure_seed_geo(paths: &AppPaths, sidecars: &Sidecars) -> Result<()> {
    std::fs::create_dir_all(&paths.geo_dir)?;
    for name in ["geoip.dat", "geosite.dat"] {
        let destination = paths.geo_dir.join(name);
        if destination.exists() {
            continue;
        }
        let source = sidecars.assets_dir().join(name);
        if source.exists() {
            std::fs::copy(&source, &destination)
                .with_context(|| format!("copy {name} into {}", paths.geo_dir.display()))?;
        }
    }
    Ok(())
}

pub fn update_geo(paths: &AppPaths, profile: &RoutingProfile, source: &str) -> Result<GeoStatus> {
    std::fs::create_dir_all(&paths.geo_dir)?;
    let (default_geoip, default_geosite) = if source == "ru" {
        (RU_GEOIP, RU_GEOSITE)
    } else {
        (DEFAULT_GEOIP, DEFAULT_GEOSITE)
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()?;
    let geoip_url = if profile.geoip_url.is_empty() {
        default_geoip
    } else {
        profile.geoip_url.as_str()
    };
    let geosite_url = if profile.geosite_url.is_empty() {
        default_geosite
    } else {
        profile.geosite_url.as_str()
    };
    download_asset(&client, geoip_url, &paths.geo_dir.join("geoip.dat"))?;
    download_asset(&client, geosite_url, &paths.geo_dir.join("geosite.dat"))?;
    // sing-box rule-sets are derived from the same upstream data; drop the
    // cache so the next sing-box backend downloads the refreshed categories.
    let _ = std::fs::remove_dir_all(paths.geo_dir.join("rulesets"));
    Ok(GeoStatus {
        geoip_bytes: std::fs::metadata(paths.geo_dir.join("geoip.dat"))?.len(),
        geosite_bytes: std::fs::metadata(paths.geo_dir.join("geosite.dat"))?.len(),
        checked_at: now_secs(),
    })
}

fn download_asset(client: &Client, url: &str, destination: &Path) -> Result<()> {
    let expected = [format!("{url}.sha256"), format!("{url}.sha256sum")]
        .iter()
        .find_map(|checksum_url| {
            client
                .get(checksum_url)
                .send()
                .ok()
                .filter(|response| response.status().is_success())
                .and_then(|response| response.text().ok())
                .map(|text| {
                    text.split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                })
                .filter(|text| text.len() == 64)
        });

    if let Some(expected) = expected.as_deref() {
        if destination.exists() {
            let existing = sha256_file(destination)?;
            if existing == expected {
                return Ok(());
            }
        }
    }

    let response = client.get(url).send().context("download geo asset")?;
    if !response.status().is_success() {
        bail!("geo download returned {}", response.status());
    }
    let bytes = response.bytes().context("read geo asset")?;
    let actual = sha256_bytes(&bytes);
    if let Some(expected) = expected {
        if actual != expected {
            bail!("geo checksum mismatch for {}", destination.display());
        }
    }
    let temp = destination.with_extension("tmp");
    std::fs::write(&temp, &bytes)?;
    std::fs::rename(&temp, destination)?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fetch_text(url: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(40))
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client.get(url).send().context("fetch routing profile")?;
    if !response.status().is_success() {
        bail!("profile server returned {}", response.status());
    }
    response.text().context("read routing profile")
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

    #[test]
    fn parses_provider_profile_and_applies_xray_rules() {
        let value = json!({
            "Name": "RU",
            "GlobalProxy": "true",
            "RemoteDNSType": "DoH",
            "RemoteDNSDomain": "https://cloudflare-dns.com/dns-query",
            "RemoteDNSIP": "1.1.1.1",
            "DirectSites": ["geosite:ru", "example.ru"],
            "DirectIp": ["geoip:ru", "10.0.0.0/8"],
            "BlockSites": ["geosite:category-ads-all"],
            "DomainStrategy": "IPIfNonMatch"
        });
        let mut config = json!({
            "inbounds": [],
            "outbounds": [
                { "tag": "proxy", "protocol": "vless", "settings": {} },
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "block", "protocol": "blackhole" }
            ],
            "routing": {
                "rules": [
                    { "type": "field", "inboundTag": ["dns"], "outboundTag": "direct" },
                    { "type": "field", "network": "tcp,udp", "balancerTag": "balancer" }
                ],
                "balancers": [{ "tag": "balancer", "selector": ["proxy"] }]
            }
        });
        let profile = profile_from_value(&value);
        apply_xray(&mut config, &profile).unwrap();
        let rules = config["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["inboundTag"][0], "dns");
        assert_eq!(rules[1]["outboundTag"], "direct");
        assert!(rules.iter().any(|rule| rule["domain"]
            .as_array()
            .map(|list| list.iter().any(|item| item == "geosite:category-ru"))
            .unwrap_or(false)));
        assert_eq!(
            config["dns"]["servers"][0],
            "https://cloudflare-dns.com/dns-query"
        );
    }

    #[test]
    fn enables_xray_fakedns() {
        let value = json!({
            "Name": "Fake",
            "FakeDNS": true,
            "RemoteDNSType": "DoH",
            "RemoteDNSDomain": "https://cloudflare-dns.com/dns-query"
        });
        let profile = profile_from_value(&value);
        let mut config = json!({
            "outbounds": [
                { "tag": "proxy", "protocol": "vless", "settings": {} },
                { "tag": "direct", "protocol": "freedom" }
            ]
        });
        apply_xray(&mut config, &profile).unwrap();
        assert_eq!(config["fakedns"]["ipPool"], "198.18.0.0/15");
        assert_eq!(config["dns"]["servers"][0], "fakedns");
    }

    #[test]
    fn applies_singbox_domains_and_skips_geo() {
        let value = json!({
            "Name": "SB",
            "DirectSites": ["example.com", "geosite:ru"],
            "BlockIp": ["1.2.3.0/24"]
        });
        let profile = profile_from_value(&value);
        let mut config = json!({ "route": { "rules": [], "final": "proxy" } });
        apply_singbox(&mut config, &profile, &[]).unwrap();
        let rules = config["route"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["action"], "sniff");
        assert_eq!(rules[1]["domain_suffix"][0], "example.com");
        assert_eq!(rules[2]["action"], "reject");
    }

    #[test]
    #[ignore = "downloads sing-box rule-sets from GitHub"]
    fn downloads_singbox_rule_sets() {
        let root = std::env::temp_dir().join(format!("m3vpn-rulesets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
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
        let value = json!({
            "DirectSites": ["geosite:ru"],
            "ProxySites": ["geosite:google"],
            "BlockIp": ["geoip:ru"]
        });
        let profile = profile_from_value(&value);
        let sets = ensure_singbox_rule_sets(&paths, &profile);
        assert_eq!(sets.len(), 3, "{sets:?}");
        assert!(sets.iter().all(|set| set.path.exists()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "requires sidecars and network access"]
    fn singbox_accepts_downloaded_rule_sets() {
        let root = std::env::temp_dir().join(format!("m3vpn-ruleset-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
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
        let value = json!({
            "DirectSites": ["geosite:ru"],
            "ProxySites": ["geosite:google"],
            "BlockIp": ["geoip:ru"]
        });
        let profile = profile_from_value(&value);
        let sets = ensure_singbox_rule_sets(&paths, &profile);
        assert_eq!(sets.len(), 3);
        let mut config = json!({
            "inbounds": [{ "type": "mixed", "tag": "in", "listen": "127.0.0.1", "listen_port": 20808 }],
            "outbounds": [
                { "type": "socks", "tag": "proxy", "server": "127.0.0.1", "server_port": 9 },
                { "type": "direct", "tag": "direct" }
            ]
        });
        apply_singbox(&mut config, &profile, &sets).unwrap();
        let path = root.join("check.json");
        std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
        let sidecars = crate::sidecars::Sidecars::discover().expect("sidecars");
        let output = std::process::Command::new(sidecars.singbox())
            .arg("check")
            .arg("-c")
            .arg(&path)
            .output()
            .expect("sing-box check");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn builds_tun_dns_from_profile() {
        let value = json!({
            "RemoteDNSType": "DoH",
            "RemoteDNSDomain": "https://dns.google/dns-query",
            "RemoteDNSIP": "8.8.8.8",
            "DomesticDNSType": "DoU",
            "DomesticDNSIP": "77.88.8.8"
        });
        let profile = profile_from_value(&value);
        let servers = tun_dns_servers(&profile);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["type"], "https");
        assert_eq!(servers[0]["server"], "dns.google");
        assert_eq!(servers[0]["path"], "/dns-query");

        let value = json!({ "RemoteDNSType": "DoU", "RemoteDNSIP": "9.9.9.9" });
        let servers = tun_dns_servers(&profile_from_value(&value));
        assert_eq!(servers[0]["type"], "udp");
        assert_eq!(servers[0]["server"], "9.9.9.9");

        let value =
            json!({ "RemoteDNSType": "DoH", "RemoteDNSDomain": "https://1.1.1.1:8443/dns-query" });
        let servers = tun_dns_servers(&profile_from_value(&value));
        assert_eq!(servers[0]["server"], "1.1.1.1");
        assert_eq!(servers[0]["server_port"], 8443);
    }

    #[test]
    fn lists_cached_rule_sets() {
        let root = std::env::temp_dir().join(format!("m3vpn-ruleset-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("geo/rulesets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("geosite-category-ru.srs"), b"abc").unwrap();
        std::fs::write(dir.join("geoip-ru.srs"), b"defg").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
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
        let sets = cached_rule_sets(&paths);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].tag, "geoip:ru");
        assert_eq!(sets[0].bytes, 4);
        assert_eq!(sets[1].tag, "geosite:category-ru");
        assert!(sets[1].modified_at.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn appends_local_rule_sets_for_singbox_geo_tags() {
        let value = json!({
            "Name": "Geo",
            "DirectSites": ["geosite:ru", "direct.example"],
            "ProxySites": ["geosite:google"],
            "BlockIp": ["geoip:ru"]
        });
        let profile = profile_from_value(&value);
        let rule_sets = vec![
            SingboxRuleSet {
                tag: "m3vpn-geosite-category-ru".into(),
                path: PathBuf::from("/tmp/geosite-category-ru.srs"),
            },
            SingboxRuleSet {
                tag: "m3vpn-geosite-google".into(),
                path: PathBuf::from("/tmp/geosite-google.srs"),
            },
            SingboxRuleSet {
                tag: "m3vpn-geoip-ru".into(),
                path: PathBuf::from("/tmp/geoip-ru.srs"),
            },
        ];
        let mut config = json!({ "route": { "rules": [], "final": "proxy" } });
        apply_singbox(&mut config, &profile, &rule_sets).unwrap();
        assert_eq!(config["route"]["rule_set"].as_array().unwrap().len(), 3);
        let rules = config["route"]["rules"].as_array().unwrap();
        assert!(rules
            .iter()
            .any(|rule| rule["rule_set"][0] == "m3vpn-geosite-category-ru"));
        assert!(rules
            .iter()
            .any(|rule| rule["rule_set"][0] == "m3vpn-geosite-google"));
        assert!(rules
            .iter()
            .any(|rule| rule["rule_set"][0] == "m3vpn-geoip-ru"));
        assert!(rules
            .iter()
            .any(|rule| rule["domain_suffix"][0] == "direct.example"));
    }

    #[test]
    fn enables_fakeip_only_for_proxied_domains() {
        let value = json!({
            "Name": "Fake",
            "FakeDNS": "true",
            "ProxySites": ["youtube.com", "geosite:google"],
            "DomesticDNSIP": "9.9.9.9"
        });
        let profile = profile_from_value(&value);
        let mut config = json!({ "route": { "rules": [], "final": "proxy" } });
        apply_singbox(&mut config, &profile, &[]).unwrap();
        let servers = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(servers[0]["tag"], "m3vpn-dns");
        assert_eq!(servers[0]["server"], "9.9.9.9");
        assert_eq!(servers[1]["type"], "fakeip");
        assert_eq!(config["dns"]["final"], "m3vpn-dns");
        assert_eq!(config["dns"]["rules"][0]["server"], "m3vpn-fakeip");
        assert_eq!(
            config["dns"]["rules"][0]["domain_suffix"],
            json!(["youtube.com"])
        );
    }

    #[test]
    fn skips_fakeip_without_proxy_domains() {
        let value = json!({ "Name": "NoFake", "FakeDNS": true, "ProxyIp": ["1.2.3.0/24"] });
        let profile = profile_from_value(&value);
        let mut config = json!({ "route": { "rules": [], "final": "proxy" } });
        apply_singbox(&mut config, &profile, &[]).unwrap();
        assert!(config.get("dns").is_none());
    }

    #[test]
    fn persists_profiles_and_tracks_active_state() {
        let dir = std::env::temp_dir().join(format!("m3vpn-routing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = store::open(&dir.join("routing.db")).unwrap();
        let value = json!({ "Name": "Saved profile", "LastUpdated": 123 });
        let profile = profile_from_value(&value);

        let id = save(&db, None, &profile, &value, None).unwrap();
        let summaries = list_profiles(&db).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Saved profile");
        assert_eq!(summaries[0].last_updated, Some(123));
        assert!(!summaries[0].active);

        set_active(&db, &id).unwrap();
        let summaries = list_profiles(&db).unwrap();
        assert!(summaries[0].active);
        assert!(geo_due(&db, &id, 1000).unwrap());
        record_geo_check(&db, &id, 1000, 2000).unwrap();
        assert!(!geo_due(&db, &id, 1500).unwrap());

        delete(&db, &id).unwrap();
        assert!(list_profiles(&db).unwrap().is_empty());
        assert_eq!(
            store::get(&db, "routing.active_profile_id")
                .unwrap()
                .as_deref(),
            Some("")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_deep_link() {
        let raw = json!({ "Name": "Deep", "GlobalProxy": false });
        let encoded = URL_SAFE_NO_PAD.encode(raw.to_string());
        let (profile, _, _) = parse_profile(&format!("m3vpn://routing/onadd/{encoded}")).unwrap();
        assert_eq!(profile.name, "Deep");
        assert!(!profile.global_proxy);
    }
}
