use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_SOCKET: &str = "/run/m3vpn/helper.sock";
const INSTALLED_HELPER: &str = "/usr/lib/m3vpn/m3vpn-helper";

/// Locates the helper binary so proxy cores can be launched through it.
/// The helper is also a small unprivileged launcher that joins the TUN proxy
/// cgroup before exec, which is required for cgroup-based packet marking.
pub fn helper_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("M3VPN_HELPER_BIN") {
        return Some(PathBuf::from(path));
    }
    let installed = PathBuf::from(INSTALLED_HELPER);
    if installed.exists() {
        return Some(installed);
    }
    let sibling = std::env::current_exe()
        .ok()?
        .parent()
        .map(|dir| dir.join("m3vpn-helper"))?;
    sibling.exists().then_some(sibling)
}

#[derive(Clone)]
pub struct HelperClient {
    socket: PathBuf,
}

impl HelperClient {
    pub fn new() -> Self {
        let socket = std::env::var("M3VPN_HELPER_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET));
        Self { socket }
    }

    pub fn at(socket: PathBuf) -> Self {
        Self { socket }
    }

    pub fn available(&self) -> bool {
        self.socket.exists()
    }

    pub fn start_tun(
        &self,
        app_pid: u32,
        front_pid: Option<u32>,
        socks_port: u16,
        kill_switch: bool,
        dns: &[Value],
        proxy_pids: &[u32],
    ) -> Result<()> {
        self.call(json!({
            "cmd": "start_tun",
            "app_pid": app_pid,
            "front_pid": front_pid,
            "socks_port": socks_port,
            "kill_switch": kill_switch,
            "dns": dns,
            "proxy_pids": proxy_pids
        }))
        .map(|_| ())
    }

    pub fn stop_tun(&self) -> Result<()> {
        self.call(json!({ "cmd": "stop_tun" })).map(|_| ())
    }

    pub fn status(&self) -> Result<bool> {
        let value = self.call(json!({ "cmd": "status" }))?;
        Ok(value["tun"].as_bool().unwrap_or(false))
    }

    fn call(&self, request: Value) -> Result<Value> {
        let mut stream = UnixStream::connect(&self.socket)
            .with_context(|| format!("connect {}", self.socket.display()))?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let line = serde_json::to_string(&request)?;
        writeln!(stream, "{line}")?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;
        let value: Value = serde_json::from_str(&response).context("parse helper response")?;
        if value["ok"].as_bool() != Some(true) {
            bail!(
                "{}",
                value["error"]
                    .as_str()
                    .unwrap_or("helper returned an error")
            );
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn status_round_trip() {
        let dir = std::env::temp_dir().join(format!("m3vpn-helper-client-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("helper.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(line.contains("status"));
                let mut stream = stream;
                stream.write_all(b"{\"ok\":true,\"tun\":false}\n").unwrap();
            }
        });
        let client = HelperClient::at(socket);
        assert!(client.available());
        assert!(!client.status().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
