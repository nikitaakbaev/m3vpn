import { create } from "zustand";
import * as api from "../lib/tauri";
import { translate, type Lang } from "../i18n";

interface AppStore {
  settings: api.Settings;
  info: api.AppInfo | null;
  loaded: boolean;
  busy: boolean;
  toast: string | null;
  load: () => Promise<void>;
  set: (key: string, value: string) => Promise<void>;
  pickPort: () => Promise<void>;
  openLogs: () => Promise<void>;
  notify: (message: string) => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  settings: { ...api.DEFAULT_SETTINGS },
  info: null,
  loaded: false,
  busy: false,
  toast: null,

  load: async () => {
    set({ busy: true });
    try {
      const [settings, info] = await Promise.all([api.getSettings(), api.getAppInfo()]);
      set({ settings, info, loaded: true, busy: false });
    } catch (error) {
      console.error(error);
      set({
        loaded: true,
        busy: false,
        toast: String(error),
      });
      void api.logFrontend("error", `failed to load settings: ${String(error)}`);
    }
  },

  set: async (key, value) => {
    set((state) => ({ settings: { ...state.settings, [key]: value } }));
    try {
      await api.setSetting(key, value);
    } catch (error) {
      set({ toast: String(error) });
      void api.logFrontend("error", `failed to save ${key}: ${String(error)}`);
    }
  },

  pickPort: async () => {
    const state = get();
    const base = Number(state.settings["proxy.port"] ?? "10808");
    try {
      const port = await api.pickFreePort(Number.isFinite(base) ? base : 10808);
      set((current) => ({
        settings: { ...current.settings, "proxy.active_port": String(port) },
        info: current.info ? { ...current.info, proxyPort: port } : current.info,
      }));
      get().notify(`${translate(get().settings["ui.language"] as Lang, "common.portFree")}: ${port}`);
    } catch (error) {
      set({ toast: String(error) });
    }
  },

  openLogs: async () => {
    try {
      await api.openLogsDir();
    } catch (error) {
      set({ toast: String(error) });
    }
  },

  notify: (message) => {
    set({ toast: message });
    window.setTimeout(() => {
      if (get().toast === message) set({ toast: null });
    }, 2600);
  },
}));

export function useT() {
  const language = useAppStore((state) => state.settings["ui.language"]);
  const lang = (language === "en" ? "en" : "ru") as Lang;
  return (key: string) => translate(lang, key);
}
