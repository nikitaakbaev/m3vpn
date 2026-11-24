import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import type { Page } from "../../components/NavRail";

interface HomePageProps {
  onNavigate: (page: Page) => void;
}

export function HomePage({ onNavigate }: HomePageProps) {
  const t = useT();
  const settings = useAppStore((state) => state.settings);
  const info = useAppStore((state) => state.info);
  const set = useAppStore((state) => state.set);
  const notify = useAppStore((state) => state.notify);
  const pickPort = useAppStore((state) => state.pickPort);

  const mode = settings["mode"] === "tun" ? "tun" : "proxy";
  const port = settings["proxy.active_port"] ?? String(info?.proxyPort ?? 10808);
  const interval = settings["ping.interval_sec"] ?? "60";
  const pingOn = settings["ping.enabled"] !== "false";
  const closeToTray = settings["startup.close_to_tray"] !== "false";

  return (
    <>
      <PageHeader
        title={t("home.title")}
        subtitle={t("home.subtitle")}
        badge={
          <span className="pill pill--success">
            <Icon name="check_circle" size={15} filled /> M0
          </span>
        }
        actions={
          <Button variant="text" icon="settings" onClick={() => onNavigate("settings")}>
            {t("home.openSettings")}
          </Button>
        }
      />

      <section className="home-grid">
        <Card variant="accent" className="connect-card">
          <div className="connect-card__top">
            <span className="pill pill--muted">
              <span className="status-dot" />
              {t("home.off")}
            </span>
            <Icon name="vpn_lock" size={30} />
          </div>

          <div className="connect-card__body">
            <div>
              <div className="eyebrow">{t("home.connection")}</div>
              <h2>{t("home.noServer")}</h2>
              <p>{t("home.connectHint")}</p>
            </div>
            <button
              type="button"
              className="connect-orb"
              aria-label={t("home.connect")}
              onClick={() => notify(t("snackbar.m1"))}
            >
              <Icon name="power_settings_new" size={42} />
            </button>
          </div>

          <div className="connect-card__footer">
            <span className="muted">{t("home.mode")}</span>
            <Segmented
              value={mode}
              label={t("home.mode")}
              options={[
                { value: "proxy", label: t("home.proxy"), icon: "public" },
                { value: "tun", label: t("home.tun"), icon: "shield" },
              ]}
              onChange={(value) => void set("mode", value)}
            />
          </div>
        </Card>

        <Card className="stat-card">
          <div className="card-heading">
            <Icon name="lan" size={20} />
            <h3>{t("home.proxyPort")}</h3>
          </div>
          <div className="metric">
            {port}
            <span>tcp</span>
          </div>
          <p className="muted">127.0.0.1:{port}</p>
          <Button variant="tonal" icon="autorenew" onClick={() => void pickPort()}>
            {t("home.pickPort")}
          </Button>
          <p className="hint">{t("home.portHint")}</p>
        </Card>

        <Card className="stat-card">
          <div className="card-heading">
            <Icon name="speed" size={20} />
            <h3>{t("home.autoPing")}</h3>
          </div>
          <div className="metric">
            {interval}
            <span>сек</span>
          </div>
          <p className="muted">{pingOn ? t("home.pingOn") : t("home.pingOff")}</p>
          <Switch
            checked={pingOn}
            label={t("home.autoPing")}
            onChange={(checked) => void set("ping.enabled", checked ? "true" : "false")}
          />
        </Card>

        <Card className="milestone-card">
          <div className="milestone">
            <span className="milestone__icon milestone__icon--done">
              <Icon name="check" size={18} filled />
            </span>
            <div>
              <h3>{t("home.m0Title")}</h3>
              <p>{t("home.m0Done")}</p>
            </div>
          </div>
          <div className="milestone">
            <span className="milestone__icon milestone__icon--active">
              <Icon name="bolt" size={18} filled />
            </span>
            <div>
              <h3>{t("home.m1Title")}</h3>
              <p>{t("home.m1Active")}</p>
            </div>
          </div>
          <div className="progress" aria-label={t("home.progress")}>
            <span style={{ width: "16%" }} />
          </div>
          <p className="hint">{t("home.progressHint")}</p>
        </Card>

        <Card className="stat-card">
          <div className="card-heading">
            <Icon name="power_settings_new" size={20} />
            <h3>{t("home.closeToTray")}</h3>
          </div>
          <p className="muted">{t("home.trayHint")}</p>
          <Switch
            checked={closeToTray}
            label={t("home.closeToTray")}
            onChange={(checked) => void set("startup.close_to_tray", checked ? "true" : "false")}
          />
        </Card>

        <Card className="stat-card stat-card--wide">
          <div className="card-heading">
            <Icon name="info" size={20} />
            <h3>{t("home.version")}</h3>
          </div>
          <dl className="facts">
            <div>
              <dt>m3vpn</dt>
              <dd>v{info?.version ?? "0.1.0"}</dd>
            </div>
            <div>
              <dt>Tauri</dt>
              <dd>{info?.tauriVersion ?? "2.x"}</dd>
            </div>
            <div>
              <dt>{t("home.stack")}</dt>
              <dd>React · M3 · SQLite</dd>
            </div>
          </dl>
        </Card>
      </section>
    </>
  );
}
