import { invoke } from "@tauri-apps/api/core";

export type Settings = Record<string, string>;

export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
  platform: string;
  dataDir: string;
  logDir: string;
  proxyPort: number;
}

export const DEFAULT_SETTINGS: Settings = {
  mode: "proxy",
  "ping.enabled": "true",
  "ping.interval_sec": "60",
  "ping.url": "https://www.gstatic.com/generate_204",
  "ping.timeout_ms": "3000",
  "ping.concurrency": "4",
  "ping.samples": "3",
  "select.mode": "auto",
  "select.tolerance_ms": "50",
  "select.tolerance_ratio": "0.3",
  "select.wins_required": "2",
  "select.failures_to_failover": "2",
  "tun.kill_switch": "false",
  "tun.strict_route": "false",
  "proxy.system": "false",
  "proxy.port": "10808",
  "proxy.auto_port": "true",
  "proxy.active_port": "10808",
  "ui.theme": "system",
  "ui.seed_color": "#4F6F52",
  "ui.language": "ru",
  "startup.autoconnect": "false",
  "startup.minimized": "false",
  "startup.close_to_tray": "true",
  "telemetry.enabled": "false",
};

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const mockInfo: AppInfo = {
  name: "m3vpn",
  version: "0.1.0",
  tauriVersion: "2.x",
  platform: "linux x86_64",
  dataDir: "~/.local/share/m3vpn",
  logDir: "~/.local/share/m3vpn/logs",
  proxyPort: 10808,
};

export async function getSettings(): Promise<Settings> {
  if (!isTauri) return { ...DEFAULT_SETTINGS };
  return invoke<Settings>("get_settings");
}

export async function setSetting(key: string, value: string): Promise<void> {
  if (!isTauri) return;
  await invoke("set_setting", { key, value });
}

export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri) return mockInfo;
  return invoke<AppInfo>("get_app_info");
}

export async function pickFreePort(start: number): Promise<number> {
  if (!isTauri) return start === 10808 || start === 10809 ? 10810 : start;
  return invoke<number>("pick_free_port", { start });
}

export async function openLogsDir(): Promise<void> {
  if (!isTauri) return;
  await invoke("open_logs_dir");
}

export async function logFrontend(level: string, message: string): Promise<void> {
  if (!isTauri) {
    console[level as "info" | "warn" | "error"](message);
    return;
  }
  await invoke("log_frontend", { level, message });
}

export function isRunningInTauri(): boolean {
  return isTauri;
}
