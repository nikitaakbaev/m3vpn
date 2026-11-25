import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Segmented } from "../../components/Segmented";
import { useAppStore, useT } from "../../store/useAppStore";
import { formatBytes, formatDate, formatUptime, parseUserinfo } from "../../lib/format";
import type { Page } from "../../components/NavRail";

interface HomePageProps {
  onNavigate: (page: Page) => void;
}

export function HomePage({ onNavigate }: HomePageProps) {
  const t = useT();
  const settings = useAppStore((state) => state.settings);
  const profiles = useAppStore((state) => state.profiles);
  const servers = useAppStore((state) => state.servers);
  const status = useAppStore((state) => state.status);
  const core = useAppStore((state) => state.core);
  const rtt = useAppStore((state) => state.rtt);
  const selectedServerId = useAppStore((state) => state.selectedServerId);
  const selectServer = useAppStore((state) => state.selectServer);
  const appInfo = useAppStore((state) => state.info);
  const set = useAppStore((state) => state.set);
  const connect = useAppStore((state) => state.connect);
  const disconnect = useAppStore((state) => state.disconnect);
  const busy = useAppStore((state) => state.busy);
  const openCoreDir = useAppStore((state) => state.openCoreDir);

  const port = status.socksPort || Number(settings["proxy.active_port"] ?? "10810") || 10810;
  const selected = servers.find((server) => server.id === selectedServerId) ?? null;
  const profile = profiles[0] ?? null;
  const userinfo = parseUserinfo(profile?.userinfo);
  const used = (userinfo.upload ?? 0) + (userinfo.download ?? 0);
  const total = userinfo.total ?? 0;
  const percent = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
  const quick = servers.slice(0, 5);
  const mode = settings["mode"] === "tun" ? "tun" : "proxy";
  const helperAvailable = appInfo?.helperAvailable ?? false;

  const changeMode = async (value: string) => {
    const next = value === "tun" ? "tun" : "proxy";
    if (next === mode) return;
    const wasConnected = status.running;
    if (wasConnected) await disconnect();
    await set("mode", next);
    if (wasConnected) await connect();
  };

  return (
    <>
      <PageHeader
        title={t("home.title")}
        subtitle={t("home.subtitle")}
        badge={<span className="pill pill--accent">M9</span>}
        actions={
          <Button variant="outlined" icon="dns" onClick={() => onNavigate("servers")}>
            {t("home.openServers")}
          </Button>
        }
      />

      <section className="hero-grid">
        <Card variant="accent" className="hero">
          <div className="hero__top">
            <span className={`pill ${status.running ? "pill--success" : "pill--muted"}`}>
              <span className={`status-dot${status.running ? " is-on" : ""}`} />
              {status.running ? t("home.on") : t("home.off")}
            </span>
            <div className="hero__mode-switch">
              <Segmented
                value={mode}
                label={t("settings.mode")}
                options={[
                  { value: "proxy", label: t("home.proxy"), icon: "public", disabled: busy },
                  {
                    value: "tun",
                    label: t("home.tun"),
                    icon: "shield",
                    disabled: !helperAvailable || busy,
                  },
                ]}
                onChange={(value) => void changeMode(value)}
              />
            </div>
          </div>

          <div className="hero__body">
            <div className="hero__copy">
              <div className="eyebrow">{t("home.connection")}</div>
              <h2>{status.running ? status.serverName ?? t("home.on") : selected?.name ?? t("home.noServer")}</h2>
              <p>{status.running ? t("home.runningHint") : t("home.readyHint")}</p>
              <div className="hero__meta">
                <span>
                  <Icon name="lan" size={16} /> 127.0.0.1:{port}
                </span>
                <span>
                  <Icon name="timer" size={16} /> {settings["ping.interval_sec"] ?? "60"}s
                </span>
                <span>
                  <Icon name="memory" size={16} /> {core?.xrayVersion?.split(" ").slice(0, 2).join(" ") ?? "Xray"}
                </span>
                {status.running && rtt !== null ? (
                  <span>
                    <Icon name="speed" size={16} /> {t("home.rtt")} {rtt} ms
                  </span>
                ) : selected?.latencyMs !== null && selected?.latencyMs !== undefined ? (
                  <span>
                    <Icon name="speed" size={16} /> {t("home.ping")} {selected.latencyMs} ms
                  </span>
                ) : null}
              </div>
            </div>

            <button
              type="button"
              className={`power${status.running ? " is-on" : ""}${busy ? " is-busy" : ""}`}
              aria-label={status.running ? t("home.disconnect") : t("home.connect")}
              disabled={busy}
              onClick={() => void (status.running ? disconnect() : connect())}
            >
              <span className="power__ring" />
              {busy ? <span className="spinner" /> : <Icon name="power_settings_new" size={40} />}
            </button>
          </div>

          <div className="hero__footer">
            <div className="hero__selected">
              <span className="muted">{t("home.selected")}</span>
              <button type="button" className="server-picker" onClick={() => onNavigate("servers")}>
                <Icon name="dns" size={17} />
                <span>{selected?.name ?? t("home.chooseServer")}</span>
                <Icon name="expand_more" size={18} />
              </button>
            </div>
            <div className="hero__actions">
              {status.running ? (
                <Button variant="outlined" icon="stop_circle" disabled={busy} onClick={() => void disconnect()}>
                  {t("home.disconnect")}
                </Button>
              ) : (
                <Button icon="bolt" disabled={busy || !selected} onClick={() => void connect()}>
                  {t("home.connect")}
                </Button>
              )}
            </div>
          </div>
        </Card>

        <div className="side-stack">
          <Card className="summary-card">
            <div className="card-heading">
              <Icon name="monitoring" size={19} />
              <h2>{t("home.traffic")}</h2>
            </div>
            {profile ? (
              <>
                <div className="summary-card__number">{formatBytes(used)}</div>
                <p className="muted">
                  {t("home.of")} {formatBytes(total)} · {t("home.until")} {formatDate(userinfo.expire)}
                </p>
                <div className="progress">
                  <span style={{ width: `${percent}%` }} />
                </div>
                <div className="summary-card__stats">
                  <span>{t("home.profilesLabel")}: {profiles.length}</span>
                  <span>{t("home.serversLabel")}: {servers.length}</span>
                </div>
              </>
            ) : (
              <>
                <p className="muted">{t("home.noSubscription")}</p>
                <Button variant="tonal" icon="add_link" onClick={() => onNavigate("subscriptions")}>
                  {t("home.addSubscription")}
                </Button>
              </>
            )}
          </Card>

          <Card className="core-card">
            <div className="card-heading">
              <Icon name="memory" size={19} />
              <h2>{t("home.core")}</h2>
            </div>
            <div className="core-card__row">
              <span className={`status-dot${core?.available ? " is-on" : ""}`} />
              <span>{core?.available ? t("home.coreReady") : t("home.coreMissing")}</span>
            </div>
            <p className="mono muted">{core?.xrayVersion ?? "xray —"}</p>
            <div className="core-card__actions">
              <Button variant="text" icon="folder_open" onClick={() => void openCoreDir()}>
                {t("home.openLogs")}
              </Button>
            </div>
          </Card>
        </div>
      </section>

      {servers.length === 0 ? (
        <Card className="welcome-card">
          <div className="welcome-card__icon">
            <Icon name="rocket_launch" size={28} />
          </div>
          <div>
            <h2>{t("home.welcomeTitle")}</h2>
            <p>{t("home.welcomeText")}</p>
          </div>
          <Button icon="add_link" onClick={() => onNavigate("subscriptions")}>
            {t("home.addSubscription")}
          </Button>
        </Card>
      ) : (
        <section className="home-lower">
          <Card className="quick-card">
            <div className="section-head">
              <div>
                <h2>{t("home.quickServers")}</h2>
                <p className="muted">{t("home.quickHint")}</p>
              </div>
              <Button variant="text" trailingIcon="arrow_forward" onClick={() => onNavigate("servers")}>
                {t("home.allServers")}
              </Button>
            </div>
            <div className="server-list">
              {quick.map((server) => {
                const active = server.id === selectedServerId;
                const connected = status.running && server.id === status.serverId;
                return (
                  <button
                    key={server.id}
                    type="button"
                    className={`server-item${active ? " is-selected" : ""}${connected ? " is-connected" : ""}`}
                    onClick={() => selectServer(server.id)}
                    onDoubleClick={() => void connect(server.id)}
                  >
                    <span className="server-item__dot" />
                    <span className="server-item__body">
                      <span className="server-item__name">{server.name}</span>
                      <span className="server-item__meta">
                        {server.protocol.toUpperCase()}
                        {server.host ? ` · ${server.host}` : ""}
                        {server.port ? `:${server.port}` : ""}
                      </span>
                    </span>
                    {connected ? (
                      <span className="pill pill--success">
                        <Icon name="check_circle" size={14} filled /> {t("home.on")}
                      </span>
                    ) : (
                      <span className="server-item__action">
                        <Icon name="play_arrow" size={18} />
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </Card>

          <Card className="details-card">
            <div className="card-heading">
              <Icon name="tune" size={19} />
              <h2>{t("home.details")}</h2>
            </div>
            <dl className="facts">
              <div>
                <dt>{t("home.status")}</dt>
                <dd>{status.running ? t("home.on") : t("home.off")}</dd>
              </div>
              <div>
                <dt>{t("home.uptime")}</dt>
                <dd>{formatUptime(status.startedAt)}</dd>
              </div>
              <div>
                <dt>PID</dt>
                <dd className="mono">{status.pid ?? "—"}</dd>
              </div>
              <div>
                <dt>{t("home.engine")}</dt>
                <dd className="mono">{status.engine ?? "—"}</dd>
              </div>
              <div>
                <dt>SOCKS</dt>
                <dd className="mono">{status.socksPort || port}</dd>
              </div>
              <div>
                <dt>HTTP</dt>
                <dd className="mono">{status.httpPort || port + 1}</dd>
              </div>
            </dl>
          </Card>
        </section>
      )}
    </>
  );
}
