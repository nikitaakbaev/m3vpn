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

export interface Profile {
  id: string;
  name: string;
  title: string | null;
  serverCount: number;
  updateIntervalHours: number;
  userinfo: string | null;
  lastRefreshAt: number | null;
  createdAt: number;
  hideUrl: boolean;
}

export interface Server {
  id: string;
  profileId: string;
  profileName: string;
  name: string;
  protocol: string;
  host: string | null;
  port: number | null;
  enabled: boolean;
  latencyMs: number | null;
}

export interface ConnectionStatus {
  running: boolean;
  serverId: string | null;
  serverName: string | null;
  socksPort: number;
  httpPort: number;
  pid: number | null;
  startedAt: number | null;
  error: string | null;
}

export interface ImportSummary {
  profileId: string;
  name: string;
  title: string | null;
  servers: number;
  skipped: number;
  notes: string[];
}

export interface CoreInfo {
  available: boolean;
  xrayPath: string | null;
  xrayVersion: string | null;
  assetsDir: string | null;
  error: string | null;
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
  "ui.theme": "dark",
  "ui.seed_color": "#22C55E",
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
  proxyPort: 10810,
};

const now = Math.floor(Date.now() / 1000);
let mockProfiles: Profile[] = [
  {
    id: "mock-profile",
    name: "PingBooster",
    title: "PingBooster",
    serverCount: 6,
    updateIntervalHours: 1,
    userinfo: "upload=0; download=9564036252; total=1288752289179; expire=1824739970",
    lastRefreshAt: now,
    createdAt: now,
    hideUrl: false,
  },
];

let mockServers: Server[] = [
  ["🇪🇺 АВТОВЫБОР · DE", "185.192.23.120", 443],
  ["🇩🇪 Germany · Reality", "185.184.128.167", 443],
  ["🇳🇱 Netherlands · Reality", "149.33.14.60", 443],
  ["🇫🇮 Finland · Reality", "95.217.130.20", 443],
  ["🇸🇪 Sweden · Reality", "45.83.91.10", 443],
  ["🇺🇸 United States · Reality", "104.28.210.5", 443],
].map(([name, host, port], index) => ({
  id: `mock-server-${index}`,
  profileId: "mock-profile",
  profileName: "PingBooster",
  name: name as string,
  protocol: "vless",
  host: host as string,
  port: port as number,
  enabled: true,
  latencyMs: null,
}));

let mockStatus: ConnectionStatus = {
  running: false,
  serverId: null,
  serverName: null,
  socksPort: 10810,
  httpPort: 10811,
  pid: null,
  startedAt: null,
  error: null,
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

export async function openCoreDir(): Promise<void> {
  if (!isTauri) return;
  await invoke("open_core_dir");
}

export async function logFrontend(level: string, message: string): Promise<void> {
  if (!isTauri) {
    console[level as "info" | "warn" | "error"](message);
    return;
  }
  await invoke("log_frontend", { level, message });
}

export async function importSubscription(url: string, name?: string): Promise<ImportSummary> {
  if (!isTauri) {
    if (!/^https?:\/\//.test(url)) throw new Error("URL подписки должен начинаться с http:// или https://");
    const profile: Profile = {
      id: `mock-${Date.now()}`,
      name: name?.trim() || "Новая подписка",
      title: name?.trim() || "Новая подписка",
      serverCount: mockServers.length,
      updateIntervalHours: 24,
      userinfo: null,
      lastRefreshAt: now,
      createdAt: now,
      hideUrl: false,
    };
    mockProfiles = [profile, ...mockProfiles];
    return {
      profileId: profile.id,
      name: profile.name,
      title: profile.title,
      servers: mockServers.length,
      skipped: 0,
      notes: [],
    };
  }
  return invoke<ImportSummary>("import_subscription", { url, name: name ?? null });
}

export async function listProfiles(): Promise<Profile[]> {
  if (!isTauri) return [...mockProfiles];
  return invoke<Profile[]>("list_profiles");
}

export async function listServers(profileId?: string): Promise<Server[]> {
  if (!isTauri) {
    return profileId ? mockServers.filter((server) => server.profileId === profileId) : [...mockServers];
  }
  return invoke<Server[]>("list_servers", { profileId: profileId ?? null });
}

export async function deleteProfile(profileId: string): Promise<void> {
  if (!isTauri) {
    mockProfiles = mockProfiles.filter((profile) => profile.id !== profileId);
    mockServers = mockServers.filter((server) => server.profileId !== profileId);
    return;
  }
  await invoke("delete_profile", { profileId });
}

export async function connectServer(serverId: string): Promise<ConnectionStatus> {
  if (!isTauri) {
    const server = mockServers.find((item) => item.id === serverId) ?? null;
    if (!server) throw new Error("сервер не найден");
    mockStatus = {
      ...mockStatus,
      running: true,
      serverId: server.id,
      serverName: server.name,
      pid: 4242,
      startedAt: Math.floor(Date.now() / 1000),
      error: null,
    };
    return mockStatus;
  }
  return invoke<ConnectionStatus>("connect_server", { serverId });
}

export async function disconnectCore(): Promise<ConnectionStatus> {
  if (!isTauri) {
    mockStatus = { ...mockStatus, running: false, serverId: null, serverName: null, pid: null, startedAt: null };
    return mockStatus;
  }
  return invoke<ConnectionStatus>("disconnect_core");
}

export async function connectionStatus(): Promise<ConnectionStatus> {
  if (!isTauri) return mockStatus;
  return invoke<ConnectionStatus>("connection_status");
}

export async function coreInfo(): Promise<CoreInfo> {
  if (!isTauri) {
    return {
      available: true,
      xrayPath: "/home/user/m3vpn/sidecars/bin/xray",
      xrayVersion: "Xray 26.11.9",
      assetsDir: "/home/user/m3vpn/sidecars/assets",
      error: null,
    };
  }
  return invoke<CoreInfo>("core_info");
}

export function isRunningInTauri(): boolean {
  return isTauri;
}
