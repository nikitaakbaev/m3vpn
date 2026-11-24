use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
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
    ("ui.theme", "system"),
    ("ui.seed_color", "#4F6F52"),
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
