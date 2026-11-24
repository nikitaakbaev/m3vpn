import { useEffect, useState } from "react";
import { NavRail, type Page } from "./components/NavRail";
import { Snackbar } from "./components/Snackbar";
import { Icon } from "./components/Icon";
import { applyTheme, watchSystemTheme, type ThemeMode } from "./theme";
import { useAppStore, useT } from "./store/useAppStore";
import { HomePage } from "./features/home/HomePage";
import { ServersPage } from "./features/servers/ServersPage";
import { SubscriptionsPage } from "./features/subscriptions/SubscriptionsPage";
import { RoutingPage } from "./features/routing/RoutingPage";
import { SettingsPage } from "./features/settings/SettingsPage";

const PAGES: Page[] = ["home", "servers", "subscriptions", "routing", "settings"];

function pageFromHash(): Page {
  const hash = window.location.hash.replace(/^#\/?/, "");
  return (PAGES as string[]).includes(hash) ? (hash as Page) : "home";
}

export default function App() {
  const load = useAppStore((state) => state.load);
  const loaded = useAppStore((state) => state.loaded);
  const settings = useAppStore((state) => state.settings);
  const t = useT();
  const [page, setPageState] = useState<Page>(pageFromHash);

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

  const theme = (settings["ui.theme"] ?? "system") as ThemeMode;
  const seed = settings["ui.seed_color"] ?? "#4F6F52";

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
    </div>
  );
}
