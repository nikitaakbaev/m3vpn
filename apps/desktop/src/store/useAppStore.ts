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
  engine: null,
  tun: false,
  error: null,
};

interface AppStore {
  settings: api.Settings;
  info: api.AppInfo | null;
  profiles: api.Profile[];
  servers: api.Server[];
  status: api.ConnectionStatus;
  core: api.CoreInfo | null;
  rtt: number | null;
  selectedServerId: string | null;
  loaded: boolean;
  busy: boolean;
  importing: boolean;
  probing: boolean;
  refreshingId: string | null;
  autoCandidateId: string | null;
  autoCandidateWins: number;
  toast: string | null;
  lastError: string | null;
  reportedStatusError: string | null;
  load: () => Promise<void>;
  refreshData: () => Promise<void>;
  pollStatus: () => Promise<void>;
  set: (key: string, value: string) => Promise<void>;
  selectServer: (serverId: string) => void;
  connect: (serverId?: string) => Promise<void>;
  disconnect: () => Promise<void>;
  importSubscription: (url: string, name?: string) => Promise<api.ImportSummary>;
  importAwg: (conf: string, name?: string) => Promise<api.ImportSummary>;
  measureRtt: () => Promise<void>;
  refreshProfile: (profileId: string) => Promise<api.ImportSummary | null>;
  deleteProfile: (profileId: string) => Promise<void>;
  probeAll: (connectFastest?: boolean, silent?: boolean) => Promise<void>;
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
  rtt: null,
  selectedServerId: null,
  loaded: false,
  busy: false,
  importing: false,
  probing: false,
  refreshingId: null,
  autoCandidateId: null,
  autoCandidateWins: 0,
  toast: null,
  lastError: null,
  reportedStatusError: null,

  load: async () => {
    try {
      const [settings, info, core] = await Promise.all([
        api.getSettings(),
        api.getAppInfo(),
        api.coreInfo(),
      ]);
      set({ settings, info, core });
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
      const [profiles, servers, status] = await Promise.all([
        api.listProfiles(),
        api.listServers(),
        api.connectionStatus(),
      ]);
      const selected = get().selectedServerId;
      const nextSelected =
        selected && servers.some((server) => server.id === selected)
          ? selected
          : servers[0]?.id ?? null;
      set({ profiles, servers, status, selectedServerId: nextSelected });
    } catch (error) {
      set({ lastError: String(error) });
      void api.logFrontend("error", `failed to refresh data: ${String(error)}`);
    }
  },

  pollStatus: async () => {
    try {
      const status = await api.connectionStatus();
      set({ status });
      if (!status.running && get().rtt !== null) {
        set({ rtt: null });
      }
      if (status.error) {
        if (status.error !== get().reportedStatusError) {
          set({ lastError: status.error, reportedStatusError: status.error });
        }
      } else if (get().reportedStatusError) {
        set({ reportedStatusError: null });
      }
    } catch {
      // The status poll must never surface a snackbar storm.
    }
  },

  set: async (key, value) => {
    set((state) => ({
      settings: { ...state.settings, [key]: value },
      ...(key === "select.mode" ? { autoCandidateId: null, autoCandidateWins: 0 } : {}),
    }));
    try {
      await api.setSetting(key, value);
    } catch (error) {
      set({ lastError: String(error) });
      void api.logFrontend("error", `failed to save ${key}: ${String(error)}`);
    }
  },

  selectServer: (serverId) =>
    set({
      selectedServerId: serverId,
      autoCandidateId: null,
      autoCandidateWins: 0,
    }),

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
      set({
        status,
        busy: false,
        selectedServerId: target,
        reportedStatusError: null,
        rtt: null,
        autoCandidateId: null,
        autoCandidateWins: 0,
      });
      state.notify(`${translate(state.settings["ui.language"] as Lang, "home.on")}: ${status.serverName ?? ""}`);
      // Give the tunnel a moment, then let auto mode check whether a clearly
      // faster server is available instead of waiting a whole ping interval.
      window.setTimeout(() => {
        void get().probeAll(false, true);
      }, 5000);
    } catch (error) {
      set({ busy: false, lastError: String(error) });
      void get().refreshData();
    }
  },

  disconnect: async () => {
    set({ busy: true });
    try {
      const status = await api.disconnectCore();
      set({
        status,
        busy: false,
        reportedStatusError: null,
        rtt: null,
        autoCandidateId: null,
        autoCandidateWins: 0,
      });
    } catch (error) {
      set({ busy: false, lastError: String(error) });
    }
  },

  importSubscription: async (url, name) => {
    set({ importing: true });
    try {
      const summary = await api.importSubscription(url, name);
      set({ importing: false });
      await get().refreshData();
      return summary;
    } catch (error) {
      // The subscriptions dialog renders import errors inline; a global dialog
      // on top of it would duplicate the same message.
      set({ importing: false });
      throw error;
    }
  },

  importAwg: async (conf, name) => {
    set({ importing: true });
    try {
      const summary = await api.importAwgConf(conf, name);
      set({ importing: false });
      await get().refreshData();
      return summary;
    } catch (error) {
      set({ importing: false });
      throw error;
    }
  },

  measureRtt: async () => {
    try {
      const rtt = await api.connectionRtt();
      set({ rtt });
    } catch {
      // RTT is best-effort; a failed probe must not disturb the UI.
    }
  },

  refreshProfile: async (profileId) => {
    set({ refreshingId: profileId, lastError: null });
    try {
      const summary = await api.refreshProfile(profileId);
      set({ refreshingId: null });
      await get().refreshData();
      get().notify(
        `${translate(get().settings["ui.language"] as Lang, "subs.imported")}: ${summary.servers}` +
          (summary.skipped
            ? ` · ${translate(get().settings["ui.language"] as Lang, "subs.skipped")}: ${summary.skipped}`
            : ""),
      );
      return summary;
    } catch (error) {
      set({ refreshingId: null, lastError: String(error) });
      return null;
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
      get().notify(translate(get().settings["ui.language"] as Lang, "subs.deleted"));
    } catch (error) {
      set({ lastError: String(error) });
    }
  },

  probeAll: async (connectFastest = false, silent = false) => {
    if (get().probing || get().busy) return;
    set({ probing: true });
    try {
      const timeout = Number(get().settings["ping.timeout_ms"] ?? "3000");
      const concurrency = Number(get().settings["ping.concurrency"] ?? "8");
      const results = await api.probeServers(
        Number.isFinite(timeout) ? timeout : 3000,
        Number.isFinite(concurrency) ? concurrency : 8,
      );
      await get().refreshData();
      const best = results
        .filter((result) => result.ok && result.latencyMs !== null)
        .sort((left, right) => (left.latencyMs ?? 0) - (right.latencyMs ?? 0))[0];
      const state = get();
      const lang = state.settings["ui.language"] as Lang;
      const autoMode = state.settings["select.mode"] !== "manual";
      const winsRequired = Math.max(
        1,
        Number(state.settings["select.wins_required"] ?? "2") || 2,
      );
      const toleranceMs = Math.max(0, Number(state.settings["select.tolerance_ms"] ?? "50") || 0);
      const toleranceRatio = Math.min(
        1,
        Math.max(0, Number(state.settings["select.tolerance_ratio"] ?? "0.3") || 0),
      );
      const current = state.servers.find(
        (server) =>
          server.id ===
          (state.status.running ? state.status.serverId : state.selectedServerId),
      );
      const currentLatency = current?.latencyMs ?? Number.POSITIVE_INFINITY;
      const bestLatency = best?.latencyMs ?? Number.POSITIVE_INFINITY;
      const clearlyBetter =
        !current ||
        bestLatency + toleranceMs < currentLatency ||
        bestLatency < currentLatency * (1 - toleranceRatio);

      if (connectFastest) {
        if (!best) {
          get().notify(translate(lang, "servers.noCandidates"));
        } else if (state.status.running && state.status.serverId === best.serverId) {
          get().selectServer(best.serverId);
          get().notify(translate(lang, "servers.alreadyFastest"));
        } else {
          get().selectServer(best.serverId);
          await get().connect(best.serverId);
        }
      } else if (autoMode && best) {
        if (!state.status.running) {
          get().selectServer(best.serverId);
        } else if (best.serverId !== current?.id && clearlyBetter) {
          const wins = state.autoCandidateId === best.serverId ? state.autoCandidateWins + 1 : 1;
          if (wins >= winsRequired) {
            set({ autoCandidateId: null, autoCandidateWins: 0 });
            await get().connect(best.serverId);
            const bestServer = state.servers.find((server) => server.id === best.serverId);
            get().notify(
              `${translate(lang, "servers.autoSwitched")}: ${bestServer?.name ?? best.serverId}`,
            );
          } else {
            set({ autoCandidateId: best.serverId, autoCandidateWins: wins });
          }
        } else {
          set({ autoCandidateId: null, autoCandidateWins: 0 });
        }
      } else {
        set({ autoCandidateId: null, autoCandidateWins: 0 });
      }

      if (!silent) {
        const ok = results.filter((result) => result.ok).length;
        get().notify(`${translate(lang, "servers.probe")}: ${ok}/${results.length}`);
      }
    } catch (error) {
      set({ lastError: String(error) });
    } finally {
      set({ probing: false });
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
