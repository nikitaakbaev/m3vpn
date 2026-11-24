use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub type Db = Connection;

const DEFAULTS: &[(&str, &str)] = &[
    ("mode", "proxy"),
    ("ping.enabled", "true"),
    ("ping.interval_sec", "60"),
    ("ping.url", "https://www.gstatic.com/generate_204"),
    ("ping.timeout_ms", "3000"),
    ("ping.concurrency", "4"),
    ("ping.samples", "3"),
    ("select.mode", "auto"),
    ("select.tolerance_ms", "50"),
    ("select.tolerance_ratio", "0.3"),
    ("select.wins_required", "2"),
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRow {
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub name: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub enabled: bool,
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
                p.userinfo_json, p.last_refresh_at, p.created_at, p.url_hidden
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
        Ok(ServerRow {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            profile_name: row.get(2)?,
            name: row.get(3)?,
            protocol: row.get(4)?,
            host: row.get(5)?,
            port: row.get::<_, Option<i64>>(6)?.map(|port| port as u16),
            enabled: row.get::<_, i64>(7)? != 0,
            latency_ms: row.get(8)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn server_config(conn: &Connection, server_id: &str) -> Result<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "SELECT name, full_config_json FROM servers WHERE id = ?1",
            [server_id],
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
}
