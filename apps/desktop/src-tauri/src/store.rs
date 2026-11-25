use anyhow::anyhow;
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub type Db = Connection;

// Keep in sync with the browser mock in apps/desktop/src/lib/tauri.ts.
const DEFAULTS: &[(&str, &str)] = &[
    ("mode", "proxy"),
    ("ping.enabled", "true"),
    ("ping.interval_sec", "60"),
    ("ping.url", "https://www.gstatic.com/generate_204"),
    ("ping.protocol", "fast_http"),
    ("ping.display", "both"),
    ("ping.timeout_ms", "3000"),
    ("ping.concurrency", "8"),
    ("ping.samples", "3"),
    ("select.mode", "auto"),
    ("select.tolerance_ms", "50"),
    ("select.tolerance_ratio", "0.15"),
    ("select.wins_required", "1"),
    ("select.failures_to_failover", "2"),
    ("tun.kill_switch", "false"),
    ("tun.strict_route", "false"),
    ("proxy.system", "false"),
    ("proxy.port", "10808"),
    ("proxy.auto_port", "true"),
    ("proxy.active_port", "10808"),
    ("ui.theme", "dark"),
    ("ui.seed_color", "#22C55E"),
    ("ui.language", "ru"),
    ("startup.autoconnect", "false"),
    ("startup.minimized", "false"),
    ("startup.close_to_tray", "true"),
    ("telemetry.enabled", "false"),
    ("routing.active_profile_id", ""),
    ("routing.geo_update_hours", "24"),
    ("servers.sort", "group"),
    ("update.github_repo", "nikitaakbaev/m3vpn"),
    ("update.autocheck", "true"),
];

pub fn open(path: &Path) -> Result<Db> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    migrate(&conn)?;
    seed(&conn)?;
    restrict_file(path)?;
    Ok(conn)
}

fn restrict_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS routing_profiles (
            id                TEXT PRIMARY KEY,
            name              TEXT NOT NULL,
            source_url        TEXT,
            auto_update       INTEGER NOT NULL DEFAULT 0,
            profile_json      TEXT NOT NULL,
            last_updated      INTEGER,
            geoip_url         TEXT,
            geosite_url       TEXT,
            geo_last_check_at INTEGER,
            geo_next_check_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS profiles (
            id                    TEXT PRIMARY KEY,
            name                  TEXT NOT NULL,
            url                   TEXT,
            url_hidden            INTEGER NOT NULL DEFAULT 0,
            title                 TEXT,
            description           TEXT,
            support_url           TEXT,
            web_page_url          TEXT,
            update_interval_hours INTEGER NOT NULL DEFAULT 24,
            userinfo_json         TEXT,
            banner_json           TEXT,
            routing_profile_id    TEXT REFERENCES routing_profiles(id),
            sort_order            TEXT NOT NULL DEFAULT 'none',
            last_refresh_at       INTEGER,
            created_at            INTEGER NOT NULL,
            updated_at            INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS servers (
            id               TEXT PRIMARY KEY,
            profile_id       TEXT REFERENCES profiles(id) ON DELETE CASCADE,
            name             TEXT NOT NULL,
            protocol         TEXT NOT NULL,
            host             TEXT,
            port             INTEGER,
            params_json      TEXT,
            raw_link         TEXT,
            raw_conf         TEXT,
            full_config_json TEXT,
            enabled          INTEGER NOT NULL DEFAULT 1,
            sort_index       INTEGER NOT NULL DEFAULT 0,
            latency_ms       INTEGER,
            latency_ok       INTEGER,
            latency_at       INTEGER,
            created_at       INTEGER NOT NULL,
            updated_at       INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_servers_profile ON servers(profile_id);

        CREATE TABLE IF NOT EXISTS server_exclusions (
            fingerprint TEXT PRIMARY KEY,
            created_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ping_samples (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id  TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
            ts         INTEGER NOT NULL,
            latency_ms INTEGER,
            ok         INTEGER NOT NULL,
            error      TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_ping_server_ts ON ping_samples(server_id, ts DESC);

        CREATE TABLE IF NOT EXISTS traffic_daily (
            day      TEXT PRIMARY KEY,
            upload   INTEGER NOT NULL DEFAULT 0,
            download INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS app_logs (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            ts      INTEGER NOT NULL,
            level   TEXT NOT NULL,
            source  TEXT NOT NULL,
            message TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn seed(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)")?;
    for (key, value) in DEFAULTS {
        stmt.execute(params![key, value])?;
    }

    // M2 switches the default look to the dark "shield" palette. Existing M1
    // databases are migrated once, without overriding later user choices.
    if get(conn, "ui.theme_migrated")?.is_none() {
        set(conn, "ui.theme", "dark")?;
        set(conn, "ui.seed_color", "#22C55E")?;
        set(conn, "ui.theme_migrated", "1")?;
    }
    // Proxy latency checks were benchmarked at concurrency 4; 8 cuts the
    // 58-server fast-HTTP run roughly in half without changing the work per server.
    if get(conn, "ping.concurrency_v2")?.is_none() {
        if get(conn, "ping.concurrency")?.as_deref() == Some("4") {
            set(conn, "ping.concurrency", "8")?;
        }
        set(conn, "ping.concurrency_v2", "1")?;
    }
    if get(conn, "ping.protocol_v2")?.is_none() {
        let current = get(conn, "ping.protocol")?.unwrap_or_default();
        if !matches!(
            current.as_str(),
            "fast_http" | "tcp" | "proxy_get" | "proxy_head"
        ) {
            set(conn, "ping.protocol", "fast_http")?;
        }
        set(conn, "ping.protocol_v2", "1")?;
    }
    // The first palette used muddy, low-contrast swatches. Move those exact
    // values to their vivid equivalents while keeping custom user colors.
    if get(conn, "ui.palette_v2")?.is_none() {
        let seed = get(conn, "ui.seed_color")?
            .unwrap_or_default()
            .to_ascii_lowercase();
        let vivid = match seed.as_str() {
            "#4f6f52" => Some("#22C55E"),
            "#3f5f8a" => Some("#2563EB"),
            "#7a4e8c" => Some("#7C3AED"),
            "#8a4e3f" => Some("#EA580C"),
            "#2f6f6b" => Some("#0F766E"),
            "#6b5b2f" => Some("#CA8A04"),
            _ => None,
        };
        if let Some(vivid) = vivid {
            set(conn, "ui.seed_color", vivid)?;
        }
        set(conn, "ui.palette_v2", "1")?;
    }

    // The first auto-switch tuning was too conservative for real TCP latencies
    // (30% and two probes). One 15% win is stable with the new hot-switch and
    // makes the feature actually visible.
    if get(conn, "select.tuned_v2")?.is_none() {
        if get(conn, "select.tolerance_ratio")?.as_deref() == Some("0.3") {
            set(conn, "select.tolerance_ratio", "0.15")?;
        }
        if get(conn, "select.wins_required")?.as_deref() == Some("2") {
            set(conn, "select.wins_required", "1")?;
        }
        set(conn, "select.tuned_v2", "1")?;
    }

    Ok(())
}

pub fn all(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (key, value) = row?;
        map.insert(key, value);
    }
    Ok(map)
}

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NewProfile {
    pub id: String,
    pub name: String,
    pub url: String,
    pub url_hidden: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub support_url: Option<String>,
    pub web_page_url: Option<String>,
    pub update_interval_hours: u32,
    pub userinfo: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewServer {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub params_json: Option<String>,
    pub full_config_json: String,
    pub sort_index: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub title: Option<String>,
    pub server_count: i64,
    pub update_interval_hours: i64,
    pub userinfo: Option<String>,
    pub last_refresh_at: Option<i64>,
    pub created_at: i64,
    pub hide_url: bool,
    pub url_present: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRow {
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub name: String,
    pub protocol: String,
    pub engine: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub enabled: bool,
    pub excluded: bool,
    pub latency_ms: Option<i64>,
}

pub fn save_import(conn: &Connection, profile: &NewProfile, servers: &[NewServer]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO profiles(
            id, name, url, url_hidden, title, description, support_url, web_page_url,
            update_interval_hours, userinfo_json, last_refresh_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            url = excluded.url,
            url_hidden = excluded.url_hidden,
            title = excluded.title,
            description = excluded.description,
            support_url = excluded.support_url,
            web_page_url = excluded.web_page_url,
            update_interval_hours = excluded.update_interval_hours,
            userinfo_json = excluded.userinfo_json,
            last_refresh_at = excluded.last_refresh_at,
            updated_at = excluded.updated_at",
        params![
            profile.id,
            profile.name,
            profile.url,
            profile.url_hidden as i64,
            profile.title,
            profile.description,
            profile.support_url,
            profile.web_page_url,
            profile.update_interval_hours as i64,
            profile.userinfo,
            profile.updated_at,
            profile.created_at,
            profile.updated_at,
        ],
    )?;

    tx.execute("DELETE FROM servers WHERE profile_id = ?1", [&profile.id])?;
    for server in servers {
        tx.execute(
            "INSERT INTO servers(
                id, profile_id, name, protocol, host, port, params_json, full_config_json,
                enabled, sort_index, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?10)",
            params![
                server.id,
                server.profile_id,
                server.name,
                server.protocol,
                server.host,
                server.port.map(i64::from),
                server.params_json,
                server.full_config_json,
                server.sort_index,
                profile.updated_at,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> Result<Vec<ProfileRow>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.title, COUNT(s.id), p.update_interval_hours,
                p.userinfo_json, p.last_refresh_at, p.created_at, p.url_hidden,
                (p.url IS NOT NULL AND p.url <> '')
         FROM profiles p
         LEFT JOIN servers s ON s.profile_id = p.id
         GROUP BY p.id
         ORDER BY p.created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ProfileRow {
            id: row.get(0)?,
            name: row.get(1)?,
            title: row.get(2)?,
            server_count: row.get(3)?,
            update_interval_hours: row.get(4)?,
            userinfo: row.get(5)?,
            last_refresh_at: row.get(6)?,
            created_at: row.get(7)?,
            hide_url: row.get::<_, i64>(8)? != 0,
            url_present: row.get::<_, i64>(9)? != 0,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_servers(conn: &Connection, profile_id: Option<&str>) -> Result<Vec<ServerRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.profile_id, p.name, s.name, s.protocol, s.host, s.port,
                s.enabled, s.latency_ms
         FROM servers s
         JOIN profiles p ON p.id = s.profile_id
         WHERE (?1 IS NULL OR s.profile_id = ?1)
         ORDER BY p.created_at DESC, s.sort_index ASC",
    )?;
    let rows = stmt.query_map([profile_id], |row| {
        let protocol: String = row.get(4)?;
        let engine = if m3vpn_parser::uses_singbox(&protocol) {
            "sing-box"
        } else {
            "xray"
        };
        Ok(ServerRow {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            profile_name: row.get(2)?,
            name: row.get(3)?,
            protocol,
            engine: engine.to_string(),
            host: row.get(5)?,
            port: row.get::<_, Option<i64>>(6)?.map(|port| port as u16),
            enabled: row.get::<_, i64>(7)? != 0,
            excluded: false,
            latency_ms: row.get(8)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    let exclusions = excluded_fingerprints(conn)?;
    for row in &mut result {
        row.excluded = exclusions.contains(&server_fingerprint(
            &row.protocol,
            row.host.as_deref(),
            row.port,
            &row.name,
        ));
    }
    Ok(result)
}

pub fn set_servers_excluded(
    conn: &Connection,
    server_ids: &[String],
    excluded: bool,
) -> Result<()> {
    for server_id in server_ids {
        set_server_excluded(conn, server_id, excluded)?;
    }
    Ok(())
}

pub fn set_server_excluded(conn: &Connection, server_id: &str, excluded: bool) -> Result<()> {
    let (protocol, host, port, name): (String, Option<String>, Option<i64>, String) = conn
        .query_row(
            "SELECT protocol, host, port, name FROM servers WHERE id = ?1",
            [server_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("сервер не найден"))?;
    let fingerprint = server_fingerprint(
        &protocol,
        host.as_deref(),
        port.map(|port| port as u16),
        &name,
    );
    if excluded {
        conn.execute(
            "INSERT INTO server_exclusions(fingerprint, created_at) VALUES (?1, ?2)
             ON CONFLICT(fingerprint) DO NOTHING",
            params![fingerprint, now_secs()],
        )?;
        conn.execute(
            "UPDATE servers SET latency_ms = NULL, latency_ok = 0, latency_at = NULL WHERE id = ?1",
            [server_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM server_exclusions WHERE fingerprint = ?1",
            [&fingerprint],
        )?;
    }
    Ok(())
}

fn excluded_fingerprints(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT fingerprint FROM server_exclusions")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut result = HashSet::new();
    for row in rows {
        result.insert(row?);
    }
    Ok(result)
}

fn server_fingerprint(protocol: &str, host: Option<&str>, port: Option<u16>, name: &str) -> String {
    format!(
        "{protocol}|{}|{}|{name}",
        host.unwrap_or_default(),
        port.unwrap_or_default()
    )
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub struct ServerDetails {
    pub name: String,
    pub protocol: String,
    pub full_config_json: String,
}

pub fn server_details(conn: &Connection, server_id: &str) -> Result<Option<ServerDetails>> {
    Ok(conn
        .query_row(
            "SELECT name, protocol, full_config_json FROM servers WHERE id = ?1",
            [server_id],
            |row| {
                Ok(ServerDetails {
                    name: row.get(0)?,
                    protocol: row.get(1)?,
                    full_config_json: row.get(2)?,
                })
            },
        )
        .optional()?)
}

pub fn update_latency(
    conn: &Connection,
    server_id: &str,
    latency_ms: Option<u64>,
    ok: bool,
    at: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE servers SET latency_ms = ?1, latency_ok = ?2, latency_at = ?3 WHERE id = ?4",
        params![
            latency_ms.map(|value| value as i64),
            ok as i64,
            at,
            server_id
        ],
    )?;
    Ok(())
}

pub fn profile_source(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<(String, Option<String>)>> {
    Ok(conn
        .query_row(
            "SELECT name, url FROM profiles WHERE id = ?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

pub fn delete_profile(conn: &Connection, profile_id: &str) -> Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", [profile_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_setting_roundtrip() {
        let path = std::env::temp_dir().join(format!("m3vpn-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).expect("open test database");

        assert_eq!(get(&conn, "mode").unwrap().as_deref(), Some("proxy"));
        set(&conn, "mode", "tun").unwrap();
        assert_eq!(get(&conn, "mode").unwrap().as_deref(), Some("tun"));
        assert!(all(&conn).unwrap().contains_key("proxy.active_port"));

        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }

    #[test]
    fn exclusions_survive_reimport_and_are_listed() {
        let path = std::env::temp_dir().join(format!("m3vpn-exclude-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).expect("open test database");
        let profile = NewProfile {
            id: "p1".into(),
            name: "Test".into(),
            url: String::new(),
            url_hidden: true,
            title: None,
            description: None,
            support_url: None,
            web_page_url: None,
            update_interval_hours: 24,
            userinfo: None,
            created_at: 1,
            updated_at: 1,
        };
        let server = NewServer {
            id: "p1:0".into(),
            profile_id: "p1".into(),
            name: "Excluded".into(),
            protocol: "vless".into(),
            host: Some("example.com".into()),
            port: Some(443),
            params_json: None,
            full_config_json: "{}".into(),
            sort_index: 0,
        };
        save_import(&conn, &profile, std::slice::from_ref(&server)).expect("save");
        assert!(!list_servers(&conn, None).unwrap()[0].excluded);

        set_servers_excluded(&conn, &["p1:0".to_string()], true).expect("exclude");
        assert!(list_servers(&conn, None).unwrap()[0].excluded);

        save_import(&conn, &profile, &[server]).expect("reimport");
        assert!(list_servers(&conn, None).unwrap()[0].excluded);

        set_server_excluded(&conn, "p1:0", false).expect("include");
        assert!(!list_servers(&conn, None).unwrap()[0].excluded);

        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }

    #[test]
    fn server_rows_expose_the_engine_that_runs_them() {
        let path = std::env::temp_dir().join(format!("m3vpn-engine-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).expect("open test database");
        let profile = NewProfile {
            id: "p1".into(),
            name: "Test".into(),
            url: String::new(),
            url_hidden: true,
            title: None,
            description: None,
            support_url: None,
            web_page_url: None,
            update_interval_hours: 24,
            userinfo: None,
            created_at: 1,
            updated_at: 1,
        };
        let servers = ["vless", "awg", "hysteria2", "tuic"]
            .iter()
            .enumerate()
            .map(|(index, protocol)| NewServer {
                id: format!("p1:{index}"),
                profile_id: "p1".into(),
                name: (*protocol).into(),
                protocol: (*protocol).into(),
                host: Some("example.com".into()),
                port: Some(443),
                params_json: None,
                full_config_json: "{}".into(),
                sort_index: index as i64,
            })
            .collect::<Vec<_>>();
        save_import(&conn, &profile, &servers).expect("save import");

        let rows = list_servers(&conn, None).expect("list servers");
        let engines = rows
            .iter()
            .map(|row| (row.protocol.as_str(), row.engine.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            engines,
            vec![
                ("vless", "xray"),
                ("awg", "sing-box"),
                ("hysteria2", "sing-box"),
                ("tuic", "sing-box"),
            ]
        );

        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }
}
