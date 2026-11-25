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
  helperAvailable: boolean;
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
  urlPresent: boolean;
}

export interface Server {
  id: string;
  profileId: string;
  profileName: string;
  name: string;
  protocol: string;
  engine: "xray" | "sing-box";
  host: string | null;
  port: number | null;
  enabled: boolean;
  excluded: boolean;
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
  engine: string | null;
  tun: boolean;
  error: string | null;
}

export interface RoutingSummary {
  id: string;
  name: string;
  sourceUrl: string | null;
  lastUpdated: number | null;
  geoLastCheckAt: number | null;
  active: boolean;
  profileJson: string;
}

export interface GeoStatus {
  geoipBytes: number;
  geositeBytes: number;
  checkedAt: number;
}

export interface UpdateArtifact {
  kind: string;
  url: string;
  sha256: string;
  size: number;
}

export interface UpdateInfo {
  configured: boolean;
  currentVersion: string;
  latestVersion: string | null;
  available: boolean;
  notes: string | null;
  publishedAt: string | null;
  artifacts: UpdateArtifact[];
  releaseUrl: string | null;
}

export interface LatencyResult {
  serverId: string;
  latencyMs: number | null;
  ok: boolean;
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
  singboxAvailable: boolean;
  singboxPath: string | null;
  singboxVersion: string | null;
  assetsDir: string | null;
  error: string | null;
}

// Browser mock only. Tauri always loads the persisted values from SQLite
// (store.rs is the source of truth for the real defaults).
const now = Math.floor(Date.now() / 1000);

let mockRoutingProfiles: RoutingSummary[] = [
  {
    id: "routing-default",
    name: "Default RU direct",
    sourceUrl: null,
    lastUpdated: now,
    geoLastCheckAt: null,
    active: true,
    profileJson: JSON.stringify(
      {
        Name: "Default RU direct",
        GlobalProxy: "true",
        RemoteDNSType: "DoH",
        RemoteDNSDomain: "https://cloudflare-dns.com/dns-query",
        RemoteDNSIP: "1.1.1.1",
        DomesticDNSType: "DoU",
        DomesticDNSIP: "8.8.8.8",
        DirectSites: ["geosite:ru"],
        DirectIp: ["geoip:ru", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
        BlockSites: ["geosite:category-ads-all"],
        DomainStrategy: "IPIfNonMatch",
      },
      null,
      2,
    ),
  },
];

export const DEFAULT_SETTINGS: Settings = {
  mode: "proxy",
  "ping.enabled": "true",
  "ping.interval_sec": "60",
  "ping.url": "https://www.gstatic.com/generate_204",
  "ping.protocol": "fast_http",
  "ping.display": "both",
  "ping.timeout_ms": "3000",
  "ping.concurrency": "8",
  "ping.samples": "3",
  "select.mode": "auto",
  "select.tolerance_ms": "50",
  "select.tolerance_ratio": "0.15",
  "select.wins_required": "1",
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
  "routing.active_profile_id": "",
  "routing.geo_update_hours": "24",
  "servers.sort": "group",
  "update.github_repo": "",
  "update.autocheck": "true",
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
  helperAvailable: true,
};

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
    urlPresent: true,
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
  engine: "xray",
  host: host as string,
  port: port as number,
  enabled: true,
  excluded: false,
  latencyMs: [38, 52, 63, 71, 88, 120][index] ?? null,
}));

mockServers.push({
  id: "mock-hysteria2",
  profileId: "mock-profile",
  profileName: "PingBooster",
  name: "🇳🇱 Netherlands · Hysteria2",
  protocol: "hysteria2",
  engine: "sing-box",
  host: "149.33.14.60",
  port: 8443,
  enabled: true,
  excluded: false,
  latencyMs: 118,
});

let mockStatus: ConnectionStatus = {
  running: false,
  serverId: null,
  serverName: null,
  socksPort: 10810,
  httpPort: 10811,
  pid: null,
  startedAt: null,
  engine: null,
  tun: false,
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
      urlPresent: true,
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

export async function importConfig(text: string, name?: string): Promise<ImportSummary> {
  if (!isTauri) {
    const trimmed = text.trim();
    if (!trimmed) throw new Error("конфиг пустой");
    const isAwg = trimmed.includes("[Interface]") || /^(amneziawg|awg):\/\//i.test(trimmed);
    const protocol = isAwg
      ? "awg"
      : /^hysteria2:\/\/|^hy2:\/\//i.test(trimmed)
        ? "hysteria2"
        : /^tuic:\/\//i.test(trimmed)
          ? "tuic"
          : "vless";
    const profile: Profile = {
      id: `mock-manual-${Date.now()}`,
      name: name?.trim() || (isAwg ? "AmneziaWG" : "Вручную"),
      title: isAwg ? "AmneziaWG" : "Вручную",
      serverCount: 1,
      updateIntervalHours: 24,
      userinfo: null,
      lastRefreshAt: now,
      createdAt: now,
      hideUrl: true,
      urlPresent: false,
    };
    mockProfiles = [profile, ...mockProfiles];
    mockServers = [
      {
        id: `${profile.id}:0`,
        profileId: profile.id,
        profileName: profile.name,
        name: profile.name,
        protocol,
        engine: ["awg", "hysteria2", "tuic"].includes(protocol) ? "sing-box" : "xray",
        host: "example.com",
        port: isAwg ? 51820 : 443,
        enabled: true,
        excluded: false,
        latencyMs: null,
      },
      ...mockServers,
    ];
    return {
      profileId: profile.id,
      name: profile.name,
      title: profile.title,
      servers: 1,
      skipped: 0,
      notes: [],
    };
  }
  return invoke<ImportSummary>("import_config", { text, name: name ?? null });
}

export async function checkUpdate(repo?: string): Promise<UpdateInfo> {
  if (!isTauri) {
    return {
      configured: Boolean(repo?.trim()),
      currentVersion: "0.1.0",
      latestVersion: "0.1.0",
      available: false,
      notes: null,
      publishedAt: null,
      artifacts: [],
      releaseUrl: null,
    };
  }
  return invoke<UpdateInfo>("check_update", { repo: repo ?? null });
}

export async function openExternal(url: string): Promise<void> {
  if (!isTauri) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await invoke("open_external", { url });
}

export async function listRoutingProfiles(): Promise<RoutingSummary[]> {
  if (!isTauri) return [...mockRoutingProfiles];
  return invoke<RoutingSummary[]>("list_routing_profiles");
}

export async function importRoutingProfile(text: string, name?: string): Promise<RoutingSummary> {
  if (!isTauri) {
    const profile: RoutingSummary = {
      id: `routing-${Date.now()}`,
      name: name?.trim() || "Imported profile",
      sourceUrl: null,
      lastUpdated: now,
      geoLastCheckAt: null,
      active: false,
      profileJson: text.trim().startsWith("{")
        ? text
        : JSON.stringify({ Name: name?.trim() || "Imported profile" }, null, 2),
    };
    mockRoutingProfiles = [profile, ...mockRoutingProfiles];
    return profile;
  }
  return invoke<RoutingSummary>("import_routing_profile", { text, name: name ?? null });
}

export async function saveRoutingProfile(
  id: string | null,
  profileJson: string,
  name?: string,
): Promise<RoutingSummary> {
  if (!isTauri) {
    const existing = mockRoutingProfiles.find((profile) => profile.id === id);
    const profile: RoutingSummary = {
      id: id ?? `routing-${Date.now()}`,
      name: name?.trim() || existing?.name || "Profile",
      sourceUrl: null,
      lastUpdated: now,
      geoLastCheckAt: null,
      active: existing?.active ?? false,
      profileJson,
    };
    mockRoutingProfiles = [profile, ...mockRoutingProfiles.filter((item) => item.id !== profile.id)];
    return profile;
  }
  return invoke<RoutingSummary>("save_routing_profile", { id, profileJson, name: name ?? null });
}

export async function deleteRoutingProfile(id: string): Promise<void> {
  if (!isTauri) {
    mockRoutingProfiles = mockRoutingProfiles.filter((profile) => profile.id !== id);
    return;
  }
  await invoke("delete_routing_profile", { id });
}

export async function activateRoutingProfile(id: string): Promise<void> {
  if (!isTauri) {
    mockRoutingProfiles = mockRoutingProfiles.map((profile) => ({
      ...profile,
      active: profile.id === id,
    }));
    return;
  }
  await invoke("activate_routing_profile", { id });
}

export async function refreshRoutingGeo(profileId?: string): Promise<GeoStatus> {
  if (!isTauri) {
    mockRoutingProfiles = mockRoutingProfiles.map((profile) =>
      profile.id === profileId || (!profileId && profile.active)
        ? { ...profile, geoLastCheckAt: now }
        : profile,
    );
    return { geoipBytes: 16_503_149, geositeBytes: 11_531_643, checkedAt: now };
  }
  return invoke<GeoStatus>("refresh_routing_geo", { profileId: profileId ?? null });
}

export async function refreshRoutingGeoIfDue(): Promise<GeoStatus | null> {
  if (!isTauri) return null;
  return invoke<GeoStatus | null>("refresh_routing_geo_if_due");
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

export async function refreshProfile(profileId: string): Promise<ImportSummary> {
  if (!isTauri) {
    const profile = mockProfiles.find((item) => item.id === profileId);
    if (!profile) throw new Error("подписка не найдена");
    profile.lastRefreshAt = Math.floor(Date.now() / 1000);
    return {
      profileId,
      name: profile.name,
      title: profile.title,
      servers: mockServers.filter((server) => server.profileId === profileId).length,
      skipped: 0,
      notes: [],
    };
  }
  return invoke<ImportSummary>("refresh_profile", { profileId });
}

export async function probeServers(
  timeoutMs = 3000,
  concurrency = 8,
  mode = "fast_http",
  testUrl = "https://www.gstatic.com/generate_204",
): Promise<LatencyResult[]> {
  if (!isTauri) {
    const results: LatencyResult[] = mockServers
      .filter((server) => !server.excluded)
      .map((server, index) => {
        const latency = 32 + index * 12 + Math.round(Math.random() * 9);
        server.latencyMs = latency;
        return { serverId: server.id, latencyMs: latency, ok: true, error: null };
      });
    return results;
  }
  return invoke<LatencyResult[]>("probe_servers", { timeoutMs, concurrency, mode, testUrl });
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
      engine: server.engine,
      tun: false,
      error: null,
    };
    return mockStatus;
  }
  return invoke<ConnectionStatus>("connect_server", { serverId });
}

export async function setServerExcluded(serverId: string, excluded: boolean): Promise<void> {
  if (!isTauri) {
    mockServers = mockServers.map((server) =>
      server.id === serverId ? { ...server, excluded, latencyMs: excluded ? null : server.latencyMs } : server,
    );
    return;
  }
  await invoke("set_server_excluded", { serverId, excluded });
}

export async function disconnectCore(): Promise<ConnectionStatus> {
  if (!isTauri) {
    mockStatus = {
      ...mockStatus,
      running: false,
      serverId: null,
      serverName: null,
      pid: null,
      startedAt: null,
      engine: null,
      tun: false,
    };
    return mockStatus;
  }
  return invoke<ConnectionStatus>("disconnect_core");
}

export async function connectionStatus(): Promise<ConnectionStatus> {
  if (!isTauri) return mockStatus;
  return invoke<ConnectionStatus>("connection_status");
}

export async function connectionRtt(): Promise<number | null> {
  if (!isTauri) return mockStatus.running ? 42 : null;
  return invoke<number | null>("connection_rtt");
}

export async function coreInfo(): Promise<CoreInfo> {
  if (!isTauri) {
    return {
      available: true,
      xrayPath: "/home/user/m3vpn/sidecars/bin/xray",
      xrayVersion: "Xray 26.11.9",
      singboxAvailable: true,
      singboxPath: "/home/user/m3vpn/sidecars/bin/hiddify-sing-box",
      singboxVersion: "hiddify-sing-box 1.13.0.h5",
      assetsDir: "/home/user/m3vpn/sidecars/assets",
      error: null,
    };
  }
  return invoke<CoreInfo>("core_info");
}

export function isRunningInTauri(): boolean {
  return isTauri;
}
