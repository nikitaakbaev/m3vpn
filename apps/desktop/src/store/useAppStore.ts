import { create } from "zustand";
import * as api from "../lib/tauri";
import { translate, type Lang } from "../i18n";

const EMPTY_STATUS: api.ConnectionStatus = {
  running: false,
  serverId: null,
  serverName: null,
  socksPort: 0,
  httpPort: 0,
  pid: null,
  startedAt: null,
  error: null,
};

interface AppStore {
  settings: api.Settings;
  info: api.AppInfo | null;
  profiles: api.Profile[];
  servers: api.Server[];
  status: api.ConnectionStatus;
  core: api.CoreInfo | null;
  selectedServerId: string | null;
  loaded: boolean;
  busy: boolean;
  importing: boolean;
  toast: string | null;
  lastError: string | null;
  load: () => Promise<void>;
  refreshData: () => Promise<void>;
  pollStatus: () => Promise<void>;
  set: (key: string, value: string) => Promise<void>;
  selectServer: (serverId: string) => void;
  connect: (serverId?: string) => Promise<void>;
  disconnect: () => Promise<void>;
  importSubscription: (url: string, name?: string) => Promise<api.ImportSummary>;
  deleteProfile: (profileId: string) => Promise<void>;
  pickPort: () => Promise<void>;
  openLogs: () => Promise<void>;
  openCoreDir: () => Promise<void>;
  notify: (message: string) => void;
  clearError: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  settings: { ...api.DEFAULT_SETTINGS },
  info: null,
  profiles: [],
  servers: [],
  status: EMPTY_STATUS,
  core: null,
  selectedServerId: null,
  loaded: false,
  busy: false,
  importing: false,
  toast: null,
  lastError: null,

  load: async () => {
    try {
      const [settings, info] = await Promise.all([api.getSettings(), api.getAppInfo()]);
      set({ settings, info });
      await get().refreshData();
      set({ loaded: true });
    } catch (error) {
      set({
        loaded: true,
        lastError: String(error),
      });
      void api.logFrontend("error", `failed to load application: ${String(error)}`);
    }
  },

  refreshData: async () => {
    try {
      const [profiles, servers, status, core] = await Promise.all([
        api.listProfiles(),
        api.listServers(),
        api.connectionStatus(),
        api.coreInfo(),
      ]);
      const selected = get().selectedServerId;
      const nextSelected =
        selected && servers.some((server) => server.id === selected)
          ? selected
          : servers[0]?.id ?? null;
      set({ profiles, servers, status, core, selectedServerId: nextSelected });
    } catch (error) {
      set({ lastError: String(error) });
      void api.logFrontend("error", `failed to refresh data: ${String(error)}`);
    }
  },

  pollStatus: async () => {
    try {
      const status = await api.connectionStatus();
      set({ status });
      if (status.error && status.error !== get().lastError) {
        set({ lastError: status.error });
      }
    } catch {
      // The status poll must never surface a snackbar storm.
    }
  },

  set: async (key, value) => {
    set((state) => ({ settings: { ...state.settings, [key]: value } }));
    try {
      await api.setSetting(key, value);
    } catch (error) {
      set({ lastError: String(error) });
      void api.logFrontend("error", `failed to save ${key}: ${String(error)}`);
    }
  },

  selectServer: (serverId) => set({ selectedServerId: serverId }),

  connect: async (serverId) => {
    const state = get();
    const target = serverId ?? state.selectedServerId ?? state.servers[0]?.id;
    if (!target) {
      state.notify(translate(state.settings["ui.language"] as Lang, "servers.emptyTitle"));
      return;
    }
    set({ busy: true, lastError: null });
    try {
      const status = await api.connectServer(target);
      set({ status, busy: false, selectedServerId: target });
      state.notify(`${translate(state.settings["ui.language"] as Lang, "home.on")}: ${status.serverName ?? ""}`);
    } catch (error) {
      set({ busy: false, lastError: String(error) });
      void get().refreshData();
    }
  },

  disconnect: async () => {
    set({ busy: true });
    try {
      const status = await api.disconnectCore();
      set({ status, busy: false });
    } catch (error) {
      set({ busy: false, lastError: String(error) });
    }
  },

  importSubscription: async (url, name) => {
    set({ importing: true, lastError: null });
    try {
      const summary = await api.importSubscription(url, name);
      set({ importing: false });
      await get().refreshData();
      return summary;
    } catch (error) {
      set({ importing: false, lastError: String(error) });
      throw error;
    }
  },

  deleteProfile: async (profileId) => {
    try {
      await api.deleteProfile(profileId);
      set((state) => ({
        selectedServerId:
          state.servers.find((server) => server.id === state.selectedServerId)?.profileId === profileId
            ? null
            : state.selectedServerId,
      }));
      await get().refreshData();
      get().notify("Подписка удалена");
    } catch (error) {
      set({ lastError: String(error) });
    }
  },

  pickPort: async () => {
    const state = get();
    const base = Number(state.settings["proxy.port"] ?? "10808");
    try {
      const port = await api.pickFreePort(Number.isFinite(base) ? base : 10808);
      set((current) => ({
        settings: { ...current.settings, "proxy.active_port": String(port) },
        status: { ...current.status, socksPort: port, httpPort: port + 1 },
        info: current.info ? { ...current.info, proxyPort: port } : current.info,
      }));
      get().notify(`${translate(get().settings["ui.language"] as Lang, "common.portFree")}: ${port}`);
    } catch (error) {
      set({ lastError: String(error) });
    }
  },

  openLogs: async () => {
    try {
      await api.openLogsDir();
    } catch (error) {
      set({ lastError: String(error) });
    }
  },

  openCoreDir: async () => {
    try {
      await api.openCoreDir();
    } catch (error) {
      set({ lastError: String(error) });
    }
  },

  notify: (message) => {
    set({ toast: message });
    window.setTimeout(() => {
      if (get().toast === message) set({ toast: null });
    }, 2600);
  },

  clearError: () => set({ lastError: null }),
}));

export function useT() {
  const language = useAppStore((state) => state.settings["ui.language"]);
  const lang = (language === "en" ? "en" : "ru") as Lang;
  return (key: string) => translate(lang, key);
}
