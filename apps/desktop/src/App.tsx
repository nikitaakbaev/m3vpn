import { useEffect, useRef, useState } from "react";
import { NavRail, type Page } from "./components/NavRail";
import { Snackbar } from "./components/Snackbar";
import { ErrorDialog } from "./components/ErrorDialog";
import { Icon } from "./components/Icon";
import { applyTheme, watchSystemTheme, type ThemeMode } from "./theme";
import { useAppStore, useT } from "./store/useAppStore";
import { HomePage } from "./features/home/HomePage";
import { ServersPage } from "./features/servers/ServersPage";
import { SubscriptionsPage } from "./features/subscriptions/SubscriptionsPage";
import { RoutingPage } from "./features/routing/RoutingPage";
import { SettingsPage } from "./features/settings/SettingsPage";

function TopbarStatus() {
  const t = useT();
  const running = useAppStore((state) => state.status.running);
  const serverName = useAppStore((state) => state.status.serverName);
  return (
    <span className={`pill ${running ? "pill--success" : "pill--muted"}`}>
      <span className={`status-dot${running ? " is-on" : ""}`} />
      {running ? serverName ?? t("topbar.connected") : t("topbar.disconnected")}
    </span>
  );
}

const PAGES: Page[] = ["home", "servers", "subscriptions", "routing", "settings"];

function pageFromHash(): Page {
  const hash = window.location.hash.replace(/^#\/?/, "");
  return (PAGES as string[]).includes(hash) ? (hash as Page) : "home";
}

export default function App() {
  const load = useAppStore((state) => state.load);
  const loaded = useAppStore((state) => state.loaded);
  const settings = useAppStore((state) => state.settings);
  const isConnected = useAppStore((state) => state.status.running);
  const serverCount = useAppStore((state) => state.servers.length);
  const connect = useAppStore((state) => state.connect);
  const probeAll = useAppStore((state) => state.probeAll);
  const measureRtt = useAppStore((state) => state.measureRtt);
  const pollStatus = useAppStore((state) => state.pollStatus);
  const t = useT();
  const [page, setPageState] = useState<Page>(pageFromHash);
  const autoTried = useRef(false);
  const theme = (settings["ui.theme"] ?? "dark") as ThemeMode;
  const seed = settings["ui.seed_color"] ?? "#22C55E";
  const pingEnabled = settings["ping.enabled"] ?? "true";
  const pingInterval = settings["ping.interval_sec"] ?? "60";
  const autoconnect = settings["startup.autoconnect"] === "true";

  const setPage = (next: Page) => {
    setPageState(next);
    if (window.location.hash !== `#${next}`) window.location.hash = next;
  };

  useEffect(() => {
    const onHashChange = () => setPageState(pageFromHash());
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!loaded) return;
    void pollStatus();
    const timer = window.setInterval(() => void pollStatus(), 2000);
    return () => window.clearInterval(timer);
  }, [loaded, pollStatus]);

  useEffect(() => {
    if (!loaded || pingEnabled !== "true" || serverCount === 0) return;
    const seconds = Math.max(5, Number(pingInterval) || 60);
    const first = window.setTimeout(() => void probeAll(false, true), 1500);
    const timer = window.setInterval(() => void probeAll(false, true), seconds * 1000);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, [loaded, pingEnabled, pingInterval, serverCount, probeAll]);

  useEffect(() => {
    if (!isConnected) return;
    void measureRtt();
    const timer = window.setInterval(() => void measureRtt(), 10_000);
    return () => window.clearInterval(timer);
  }, [isConnected, measureRtt]);

  useEffect(() => {
    if (!loaded || autoTried.current) return;
    if (!autoconnect) return;
    if (isConnected || serverCount === 0) return;
    autoTried.current = true;
    void connect();
  }, [loaded, autoconnect, isConnected, serverCount, connect]);

  useEffect(() => {
    applyTheme(seed, theme);
    if (theme !== "system") return;
    return watchSystemTheme(() => applyTheme(seed, theme));
  }, [seed, theme]);

  if (!loaded) {
    return (
      <div className="splash">
        <div className="splash__logo">
          <Icon name="hub" size={40} filled />
        </div>
        <div className="splash__text">
          <strong>m3vpn</strong>
          <span>Material 3 · Linux desktop</span>
        </div>
        <div className="splash__bar">
          <span />
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <NavRail page={page} onNavigate={setPage} />
      <div className="app__main">
        <header className="topbar" data-tauri-drag-region>
          <div className="topbar__brand" data-tauri-drag-region>
            <span className="topbar__name">m3vpn</span>
            <span className="topbar__tag">Material 3 · Linux</span>
          </div>
          <div className="topbar__actions">
            <TopbarStatus />
            <span className="pill pill--tonal">{t("topbar.badge")}</span>
          </div>
        </header>
        <main className="content">
          {page === "home" ? <HomePage onNavigate={setPage} /> : null}
          {page === "servers" ? <ServersPage onNavigate={setPage} /> : null}
          {page === "subscriptions" ? <SubscriptionsPage /> : null}
          {page === "routing" ? <RoutingPage /> : null}
          {page === "settings" ? <SettingsPage /> : null}
        </main>
      </div>
      <Snackbar />
      <ErrorDialog />
    </div>
  );
}
