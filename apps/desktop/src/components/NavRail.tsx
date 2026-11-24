import { Icon } from "./Icon";
import { useAppStore, useT } from "../store/useAppStore";

export type Page = "home" | "servers" | "subscriptions" | "routing" | "settings";

interface NavRailProps {
  page: Page;
  onNavigate: (page: Page) => void;
}

const ITEMS: { id: Page; icon: string; key: string }[] = [
  { id: "home", icon: "home", key: "nav.home" },
  { id: "servers", icon: "dns", key: "nav.servers" },
  { id: "subscriptions", icon: "playlist_add", key: "nav.subscriptions" },
  { id: "routing", icon: "route", key: "nav.routing" },
  { id: "settings", icon: "settings", key: "nav.settings" },
];

export function NavRail({ page, onNavigate }: NavRailProps) {
  const t = useT();
  const info = useAppStore((state) => state.info);

  return (
    <nav className="nav-rail" aria-label={t("app.name")}>
      <div className="nav-rail__brand" title={t("app.tagline")}>
        <Icon name="hub" size={26} filled />
      </div>

      <div className="nav-rail__items">
        {ITEMS.map((item) => {
          const active = item.id === page;
          return (
            <button
              key={item.id}
              type="button"
              className={`nav-rail__item${active ? " is-active" : ""}`}
              aria-current={active ? "page" : undefined}
              onClick={() => onNavigate(item.id)}
            >
              <span className="nav-rail__indicator" />
              <Icon name={item.icon} size={22} filled={active} />
              <span className="nav-rail__label">{t(item.key)}</span>
            </button>
          );
        })}
      </div>

      <div className="nav-rail__footer">
        <span className="nav-rail__version">v{info?.version ?? "0.1.0"}</span>
      </div>
    </nav>
  );
}
