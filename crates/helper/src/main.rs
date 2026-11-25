use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const IFACE: &str = "m3vpn0";
const DEFAULT_SOCKET: &str = "/run/m3vpn/helper.sock";
const DEFAULT_RUN_DIR: &str = "/run/m3vpn";
const DEFAULT_SINGBOX: &str = "/usr/lib/m3vpn/sidecars/bin/hiddify-sing-box";
const FRONT_MARK: &str = "0x2022";
const MARK_TABLE: &str = "m3vpn_mark";

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    StartTun {
        app_pid: u32,
        front_pid: Option<u32>,
        socks_port: u16,
        #[serde(default)]
        kill_switch: bool,
        #[serde(default)]
        dns: Vec<Value>,
        #[serde(default)]
        proxy_pids: Vec<u32>,
    },
    StopTun,
    Status,
    Panic,
    Restore,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    tun: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    iface: Option<String>,
}

impl Response {
    fn ok(tun: bool) -> Self {
        Self {
            ok: true,
            error: None,
            tun,
            iface: if tun { Some(IFACE.to_string()) } else { None },
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(message.into()),
            tun: false,
            iface: None,
        }
    }
}

struct TunSession {
    child: Child,
    kill_switch: bool,
}

type Shared = Arc<Mutex<Option<TunSession>>>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("run");
    let result = match command {
        "run" => daemon(),
        "restore" => restore_from_disk(&run_dir()),
        "status" => client_command(json!({ "cmd": "status" })).map(|value| print_json(&value)),
        "stop" => client_command(json!({ "cmd": "stop_tun" })).map(|value| print_json(&value)),
        "panic" => client_command(json!({ "cmd": "panic" })).map(|value| print_json(&value)),
        "start-tun" => client_command(parse_start_args(&args)).map(|value| print_json(&value)),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("m3vpn-helper: {error:#}");
        std::process::exit(1);
    }
}

fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn print_help() {
    println!(
        "m3vpn-helper run | status | stop | panic | restore | start-tun --app-pid N --socks-port N [--proxy-pid N ...] [--front-pid N] [--kill-switch]"
    );
}

fn daemon() -> Result<()> {
    unsafe {
        libc::umask(0o007);
    }
    let run_dir = run_dir();
    fs::create_dir_all(&run_dir)?;
    set_mode(&run_dir, 0o770)?;

    // Recover from a helper/app crash before accepting new commands.
    let _ = restore_from_disk(&run_dir);

    let socket = socket_path();
    let _ = fs::remove_file(&socket);
    let listener =
        UnixListener::bind(&socket).with_context(|| format!("bind {}", socket.display()))?;
    set_mode(&socket, 0o660)?;
    eprintln!("m3vpn-helper listening on {}", socket.display());

    let state: Shared = Arc::new(Mutex::new(None));
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let run_dir = run_dir.clone();
                std::thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, state, run_dir) {
                        eprintln!("m3vpn-helper request failed: {error:#}");
                    }
                });
            }
            Err(error) => eprintln!("m3vpn-helper accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(stream: UnixStream, state: Shared, run_dir: PathBuf) -> Result<()> {
    // Access is enforced by the socket group/mode (root:m3vpn, 0660).
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let request: Request = serde_json::from_str(&line).context("parse request")?;

    let response = match request {
        Request::StartTun {
            app_pid,
            front_pid,
            socks_port,
            kill_switch,
            dns,
            proxy_pids,
        } => start_tun(
            &state,
            &run_dir,
            app_pid,
            front_pid,
            socks_port,
            kill_switch,
            &dns,
            &proxy_pids,
        ),
        Request::StopTun => stop_tun(&state, &run_dir),
        Request::Status => {
            let session = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Ok(Response::ok(session.is_some()))
        }
        Request::Panic | Request::Restore => {
            stop_tun(&state, &run_dir)?;
            restore_from_disk(&run_dir)?;
            Ok(Response::ok(false))
        }
    }
    .unwrap_or_else(|error| Response::error(format!("{error:#}")));

    let mut writer = stream;
    serde_json::to_writer(&mut writer, &response)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn start_tun(
    state: &Shared,
    run_dir: &Path,
    app_pid: u32,
    front_pid: Option<u32>,
    socks_port: u16,
    kill_switch: bool,
    dns: &[Value],
    proxy_pids: &[u32],
) -> Result<Response> {
    {
        let session = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if session.is_some() {
            bail!("TUN already running");
        }
    }
    if !process_exists(app_pid) {
        bail!("app process {app_pid} is not running");
    }
    if let Some(front_pid) = front_pid {
        if !process_exists(front_pid) {
            bail!("front proxy process {front_pid} is not running");
        }
    }
    if socks_port < 1024 {
        bail!("invalid SOCKS port");
    }

    snapshot(run_dir)?;
    let bypass = install_front_bypass()?;
    if let Err(error) = install_proxy_mark(app_pid, proxy_pids) {
        drop(bypass);
        restore_network(run_dir);
        return Err(error);
    }
    let config_path = run_dir.join("tun.json");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&tun_config(socks_port, dns))?,
    )?;
    set_mode(&config_path, 0o600)?;

    let binary = singbox_path()?;
    let log = File::create(run_dir.join("tun.log"))?;
    let stdout = log.try_clone()?;
    let mut child = Command::new(&binary)
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log))
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;

    if let Err(error) = wait_for_interface(&mut child, run_dir) {
        let _ = child.kill();
        let _ = child.wait();
        cleanup_tun_interface();
        restore_network(run_dir);
        return Err(error);
    }
    if kill_switch {
        if let Err(error) = apply_kill_switch() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = remove_kill_switch();
            cleanup_tun_interface();
            restore_network(run_dir);
            return Err(error);
        }
    }

    let active = json!({
        "pid": child.id(),
        "app_pid": app_pid,
        "front_pid": front_pid,
        "kill_switch": kill_switch
    });
    let active_result: Result<()> = (|| {
        fs::write(
            run_dir.join("active.json"),
            serde_json::to_string_pretty(&active)?,
        )?;
        set_mode(&run_dir.join("active.json"), 0o600)?;
        Ok(())
    })();
    if let Err(error) = active_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = remove_kill_switch();
        cleanup_tun_interface();
        restore_network(run_dir);
        return Err(error);
    }

    {
        let mut session = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *session = Some(TunSession { child, kill_switch });
    }
    std::mem::forget(bypass);

    let state = Arc::clone(state);
    let run_dir = run_dir.to_path_buf();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let gone =
            !process_exists(app_pid) || front_pid.map(|pid| !process_exists(pid)).unwrap_or(false);
        if gone {
            eprintln!("m3vpn-helper: app/front gone, restoring network");
            let _ = stop_tun(&state, &run_dir);
            break;
        }
        let (running, core_exited) = {
            let mut session = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match session.as_mut() {
                Some(session) => (
                    true,
                    matches!(session.child.try_wait(), Ok(Some(_)) | Err(_)),
                ),
                None => (false, false),
            }
        };
        if core_exited {
            eprintln!("m3vpn-helper: TUN core exited, restoring network");
            let _ = stop_tun(&state, &run_dir);
            break;
        }
        if !running {
            break;
        }
    });

    eprintln!("m3vpn-helper: TUN {IFACE} started via {socks_port}");
    Ok(Response::ok(true))
}

fn stop_tun(state: &Shared, run_dir: &Path) -> Result<Response> {
    let session = {
        let mut session = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        session.take()
    };

    if let Some(mut session) = session {
        let _ = session.child.kill();
        let _ = session.child.wait();
        if session.kill_switch {
            let _ = remove_kill_switch();
        }
        eprintln!("m3vpn-helper: TUN stopped");
    } else {
        let _ = remove_kill_switch();
    }

    cleanup_tun_interface();
    restore_network(run_dir);
    let _ = fs::remove_file(run_dir.join("active.json"));
    Ok(Response::ok(false))
}

fn restore_from_disk(run_dir: &Path) -> Result<()> {
    let active_path = run_dir.join("active.json");
    if let Ok(text) = fs::read_to_string(&active_path) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let Some(pid) = value["pid"].as_u64() {
                terminate(pid as i32);
            }
        }
    }
    let _ = remove_kill_switch();
    cleanup_tun_interface();
    restore_network(run_dir);
    let _ = fs::remove_file(active_path);
    Ok(())
}

fn snapshot(run_dir: &Path) -> Result<()> {
    let dir = run_dir.join("snapshot");
    fs::create_dir_all(&dir)?;
    capture(
        "ip",
        &["-j", "route", "show", "table", "all"],
        &dir.join("routes.json"),
    )?;
    let defaults = capture_text("ip", &["route", "show", "default"])?;
    fs::write(dir.join("routes-default.txt"), defaults)?;
    let defaults6 = capture_text("ip", &["-6", "route", "show", "default"])?;
    fs::write(dir.join("routes6-default.txt"), defaults6)?;
    let _ = capture_text("nft", &["list", "ruleset"])
        .map(|text| fs::write(dir.join("nft.ruleset"), text));
    let _ =
        capture_text("iptables-save", &[]).map(|text| fs::write(dir.join("iptables.rules"), text));
    let _ = fs::copy("/etc/resolv.conf", dir.join("resolv.conf"));
    let _ = capture_text("resolvectl", &["status"])
        .map(|text| fs::write(dir.join("resolvectl.txt"), text));
    Ok(())
}

struct FrontBypass;

impl Drop for FrontBypass {
    fn drop(&mut self) {
        remove_front_bypass();
    }
}

fn install_front_bypass() -> Result<FrontBypass> {
    let _ = Command::new("ip")
        .args([
            "rule", "del", "fwmark", FRONT_MARK, "lookup", "main", "priority", "8999",
        ])
        .output();
    let output = Command::new("ip")
        .args([
            "rule", "add", "fwmark", FRONT_MARK, "lookup", "main", "priority", "8999",
        ])
        .output()
        .context("add TUN bypass rule")?;
    if !output.status.success() {
        bail!(
            "ip rule add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(FrontBypass)
}

fn remove_front_bypass() {
    let _ = Command::new("ip")
        .args([
            "rule", "del", "fwmark", FRONT_MARK, "lookup", "main", "priority", "8999",
        ])
        .output();
}

fn cleanup_tun_interface() {
    let _ = Command::new("ip").args(["link", "delete", IFACE]).output();
    remove_proxy_mark();
    remove_front_bypass();
    for priority in 9000..=9010 {
        loop {
            let output = Command::new("ip")
                .args(["rule", "del", "priority", &priority.to_string()])
                .output();
            if !output
                .map(|output| output.status.success())
                .unwrap_or(false)
            {
                break;
            }
        }
    }
    let _ = Command::new("ip")
        .args(["route", "flush", "table", "2022"])
        .output();
}

/// Marks the app's own systemd cgroup so front/backend transport bypasses TUN.
/// The app and all cores it spawns live in this cgroup already, so no cgroup
/// creation or pre-exec launcher is needed. The leaf must be an app-specific
/// service/scope; marking a broad user manager cgroup would bypass TUN for
/// every user process.
fn install_proxy_mark(app_pid: u32, proxy_pids: &[u32]) -> Result<()> {
    let app_cgroup = cgroup_path(app_pid)?;
    let leaf = app_cgroup
        .rsplit('/')
        .next()
        .filter(|leaf| !leaf.is_empty())
        .ok_or_else(|| anyhow!("/proc/{app_pid}/cgroup has no v2 path"))?;
    if leaf.contains("user@") || !(leaf.ends_with(".service") || leaf.ends_with(".scope")) {
        bail!(
            "app cgroup {app_cgroup} is too broad for TUN marking; launch m3vpn from the desktop"
        );
    }
    for pid in proxy_pids {
        let path = cgroup_path(*pid)?;
        if path != app_cgroup && !path.starts_with(&format!("{app_cgroup}/")) {
            bail!("proxy process {pid} is outside the app cgroup {app_cgroup}");
        }
    }
    let relative = app_cgroup.trim_start_matches('/');
    let components: Vec<&str> = relative.split('/').collect();
    if components.is_empty() {
        bail!("invalid app cgroup {app_cgroup}");
    }
    if !relative
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '@' | ':' | '/'))
    {
        bail!("unsupported characters in app cgroup {app_cgroup}");
    }
    let level = components.len().to_string();
    let path = format!("\"{relative}\"");
    remove_proxy_mark();
    let chain = "{ type route hook output priority -150; }";
    let commands: Vec<Vec<String>> = vec![
        vec![
            "add".into(),
            "table".into(),
            "inet".into(),
            MARK_TABLE.into(),
        ],
        vec![
            "add".into(),
            "chain".into(),
            "inet".into(),
            MARK_TABLE.into(),
            "output".into(),
            chain.into(),
        ],
        vec![
            "add".into(),
            "rule".into(),
            "inet".into(),
            MARK_TABLE.into(),
            "output".into(),
            "socket".into(),
            "cgroupv2".into(),
            "level".into(),
            level,
            path,
            "meta".into(),
            "mark".into(),
            "set".into(),
            FRONT_MARK.into(),
        ],
    ];
    for args in commands {
        let output = Command::new("nft")
            .args(&args)
            .output()
            .context("run nft")?;
        if !output.status.success() {
            remove_proxy_mark();
            bail!(
                "nft mark rule failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(())
}

fn cgroup_path(pid: u32) -> Result<String> {
    let text = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .with_context(|| format!("read /proc/{pid}/cgroup"))?;
    let path = text
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or_else(|| anyhow!("process {pid} is not in a cgroup v2 hierarchy"))?;
    if path == "/" {
        bail!("process {pid} is in the root cgroup; cannot scope TUN transport");
    }
    Ok(path.to_string())
}

fn remove_proxy_mark() {
    let _ = Command::new("nft")
        .args(["delete", "table", "inet", MARK_TABLE])
        .output();
}

fn restore_network(run_dir: &Path) {
    let dir = run_dir.join("snapshot");
    restore_defaults(&dir.join("routes-default.txt"), false);
    restore_defaults(&dir.join("routes6-default.txt"), true);
    let _ = Command::new("timeout")
        .args(["2", "resolvectl", "revert", IFACE])
        .output();
}

fn restore_defaults(path: &Path, ipv6: bool) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let mut command = Command::new("ip");
        if ipv6 {
            command.arg("-6");
        }
        command.arg("route").arg("replace").args(tokens);
        let _ = command.output();
    }
}

fn wait_for_interface(child: &mut Child, run_dir: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let tail = tail_file(&run_dir.join("tun.log"), 25);
            bail!("TUN core exited ({status}).\n{tail}");
        }
        if Command::new("ip")
            .args(["link", "show", IFACE])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let tail = tail_file(&run_dir.join("tun.log"), 25);
            bail!("TUN interface {IFACE} did not appear.\n{tail}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn tun_config(socks_port: u16, dns: &[Value]) -> Value {
    let (dns_servers, dns_final) = tun_dns_servers(dns);
    json!({
        "log": { "level": "warn", "timestamp": true },
        "dns": {
            "servers": dns_servers,
            "final": dns_final
        },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": IFACE,
            "address": ["172.19.0.1/30", "fdfe:dcba:9876::1/126"],
            "auto_route": true,
            "strict_route": false,
            "stack": "mixed",
            "mtu": 9000
        }],
        "outbounds": [
            {
                "type": "socks",
                "tag": "proxy",
                "server": "127.0.0.1",
                "server_port": socks_port
            },
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": [
                { "action": "sniff" },
                { "protocol": "dns", "action": "hijack-dns" },
                { "ip_is_private": true, "outbound": "direct" }
            ],
            "final": "proxy",
            "auto_detect_interface": true,
            "default_mark": 0x2022
        }
    })
}

/// Builds the DNS section for the TUN core. The app may pass DoH/DoU servers
/// from the active routing profile; a proxied UDP bootstrap resolves their
/// hostnames without leaking queries. Invalid entries are ignored so a broken
/// profile can never prevent TUN from starting.
fn tun_dns_servers(dns: &[Value]) -> (Vec<Value>, String) {
    let mut servers = vec![json!({
        "tag": "dns-bootstrap",
        "type": "https",
        "server": "1.1.1.1",
        "path": "/dns-query",
        "tls": { "server_name": "cloudflare-dns.com" },
        "detour": "proxy"
    })];
    let mut final_tag = "dns-bootstrap".to_string();
    for (index, raw) in dns.iter().enumerate() {
        let Some(object) = raw.as_object() else {
            continue;
        };
        let kind = object.get("type").and_then(Value::as_str).unwrap_or("udp");
        if !matches!(kind, "udp" | "tcp" | "tls" | "https" | "quic") {
            continue;
        }
        let Some(server) = object.get("server").and_then(Value::as_str) else {
            continue;
        };
        if server.is_empty() {
            continue;
        }
        let tag = format!("dns-{index}");
        let mut value = object.clone();
        value.insert("tag".into(), json!(tag));
        value.insert("detour".into(), json!("proxy"));
        if server.parse::<std::net::IpAddr>().is_err() {
            value.insert("domain_resolver".into(), json!("dns-bootstrap"));
        }
        servers.push(Value::Object(value));
        if index == 0 {
            final_tag = tag;
        }
    }
    (servers, final_tag)
}

fn apply_kill_switch() -> Result<()> {
    let commands: &[&[&str]] = &[
        &["add", "table", "inet", "m3vpn"],
        &[
            "add",
            "chain",
            "inet",
            "m3vpn",
            "output",
            "{ type filter hook output priority 0; policy drop; }",
        ],
        &[
            "add", "rule", "inet", "m3vpn", "output", "oifname", "lo", "accept",
        ],
        &[
            "add",
            "rule",
            "inet",
            "m3vpn",
            "output",
            "ip",
            "daddr",
            "{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16 }",
            "accept",
        ],
        &[
            "add",
            "rule",
            "inet",
            "m3vpn",
            "output",
            "ip6",
            "daddr",
            "{ fc00::/7, fe80::/10 }",
            "accept",
        ],
        &[
            "add", "rule", "inet", "m3vpn", "output", "meta", "mark", FRONT_MARK, "accept",
        ],
        &[
            "add",
            "rule",
            "inet",
            "m3vpn",
            "output",
            "ct",
            "state",
            "established,related",
            "accept",
        ],
        &[
            "add", "rule", "inet", "m3vpn", "output", "meta", "skuid", "0", "accept",
        ],
    ];
    for args in commands {
        if let Err(error) = Command::new("nft").args(*args).output() {
            let _ = remove_kill_switch();
            bail!("nft failed: {error}");
        }
    }
    Ok(())
}

fn remove_kill_switch() -> Result<()> {
    let _ = Command::new("nft")
        .args(["delete", "table", "inet", "m3vpn"])
        .output();
    Ok(())
}

fn terminate(pid: i32) {
    if !process_exists(pid as u32) {
        return;
    }
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_millis(400));
    if process_exists(pid as u32) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
}

fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn run_dir() -> PathBuf {
    std::env::var("M3VPN_RUN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_RUN_DIR))
}

fn socket_path() -> PathBuf {
    std::env::var("M3VPN_HELPER_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET))
}

fn singbox_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("M3VPN_SINGBOX") {
        return Ok(PathBuf::from(path));
    }
    let installed = PathBuf::from(DEFAULT_SINGBOX);
    if installed.exists() {
        return Ok(installed);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("hiddify-sing-box");
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }
    bail!("hiddify-sing-box not found; set M3VPN_SINGBOX")
}

fn capture(program: &str, args: &[&str], path: &Path) -> Result<()> {
    let text = capture_text(program, args)?;
    fs::write(path, text)?;
    Ok(())
}

fn capture_text(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("run {program}"))?;
    if !output.status.success() {
        bail!("{program} exited with {}", output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn tail_file(path: &Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let collected: Vec<&str> = text.lines().rev().take(lines).collect();
    collected.into_iter().rev().collect::<Vec<_>>().join("\n")
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn parse_start_args(args: &[String]) -> Value {
    let mut app_pid = 0u32;
    let mut front_pid: Option<u32> = None;
    let mut socks_port = 0u16;
    let mut kill_switch = false;
    let mut proxy_pids: Vec<u32> = Vec::new();
    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--app-pid" => app_pid = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--front-pid" => {
                front_pid = iter.next().and_then(|v| v.parse().ok());
            }
            "--socks-port" => socks_port = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--kill-switch" => kill_switch = true,
            "--proxy-pid" => {
                if let Some(pid) = iter.next().and_then(|v| v.parse().ok()) {
                    proxy_pids.push(pid);
                }
            }
            _ => {}
        }
    }
    json!({
        "cmd": "start_tun",
        "app_pid": app_pid,
        "front_pid": front_pid,
        "socks_port": socks_port,
        "kill_switch": kill_switch,
        "proxy_pids": proxy_pids
    })
}

fn client_command(request: Value) -> Result<Value> {
    let socket = socket_path();
    let mut stream =
        UnixStream::connect(&socket).with_context(|| format!("connect {}", socket.display()))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let line = serde_json::to_string(&request)?;
    writeln!(stream, "{line}")?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    let value: Value = serde_json::from_str(&response)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tun_config_routes_through_front_socks() {
        let config = tun_config(10808, &[]);
        assert_eq!(config["inbounds"][0]["type"], "tun");
        assert_eq!(config["inbounds"][0]["interface_name"], IFACE);
        assert_eq!(config["outbounds"][0]["type"], "socks");
        assert_eq!(config["outbounds"][0]["server_port"], 10808);
        assert_eq!(config["route"]["final"], "proxy");
        assert_eq!(config["route"]["default_mark"], 0x2022);
        assert_eq!(config["dns"]["final"], "dns-bootstrap");
        assert_eq!(config["dns"]["servers"][0]["type"], "https");
    }

    #[test]
    fn tun_config_uses_profile_dns() {
        let dns = vec![
            json!({ "type": "https", "server": "cloudflare-dns.com", "path": "/dns-query" }),
            json!({ "type": "udp", "server": "9.9.9.9" }),
            json!({ "type": "bogus", "server": "1.0.0.1" }),
        ];
        let config = tun_config(10808, &dns);
        let servers = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 3);
        assert_eq!(config["dns"]["final"], "dns-0");
        assert_eq!(servers[1]["domain_resolver"], "dns-bootstrap");
        assert_eq!(servers[1]["detour"], "proxy");
        assert_eq!(servers[2]["server"], "9.9.9.9");
        assert!(servers[2].get("domain_resolver").is_none());
    }

    #[test]
    #[ignore = "requires the sing-box sidecar"]
    fn tun_config_is_valid_for_singbox() {
        let dns = vec![
            json!({ "type": "https", "server": "cloudflare-dns.com", "path": "/dns-query" }),
            json!({ "type": "udp", "server": "9.9.9.9" }),
        ];
        let config = tun_config(10808, &dns);
        let dir = std::env::temp_dir().join(format!("m3vpn-tun-check-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tun.json");
        std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
        let binary = std::env::var("M3VPN_SINGBOX")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../sidecars/bin/hiddify-sing-box")
            });
        let output = Command::new(binary)
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn start_arguments_are_parsed() {
        let args = vec![
            "m3vpn-helper".to_string(),
            "start-tun".to_string(),
            "--app-pid".to_string(),
            "42".to_string(),
            "--socks-port".to_string(),
            "10808".to_string(),
            "--kill-switch".to_string(),
        ];
        let request = parse_start_args(&args);
        assert_eq!(request["app_pid"], 42);
        assert_eq!(request["socks_port"], 10808);
        assert_eq!(request["kill_switch"], true);
    }
}
