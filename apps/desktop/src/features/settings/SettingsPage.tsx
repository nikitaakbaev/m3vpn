import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { Field } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import type { ThemeMode } from "../../theme";

const SWATCHES = ["#4F6F52", "#3F5F8A", "#7A4E8C", "#8A4E3F", "#2F6F6B", "#6B5B2F"];

export function SettingsPage() {
  const t = useT();
  const settings = useAppStore((state) => state.settings);
  const info = useAppStore((state) => state.info);
  const set = useAppStore((state) => state.set);
  const pickPort = useAppStore((state) => state.pickPort);
  const openLogs = useAppStore((state) => state.openLogs);
  const openCoreDir = useAppStore((state) => state.openCoreDir);
  const core = useAppStore((state) => state.core);
  const status = useAppStore((state) => state.status);
  const notify = useAppStore((state) => state.notify);

  const theme = (settings["ui.theme"] ?? "dark") as ThemeMode;
  const seed = settings["ui.seed_color"] ?? "#22C55E";
  const language = settings["ui.language"] === "en" ? "en" : "ru";
  const mode = settings["mode"] === "tun" ? "tun" : "proxy";
  const autoPort = settings["proxy.auto_port"] !== "false";
  const activePort = settings["proxy.active_port"] ?? String(info?.proxyPort ?? 10808);
  const pingEnabled = settings["ping.enabled"] !== "false";
  const selectMode = settings["select.mode"] === "manual" ? "manual" : "auto";

  const saveNumber = (key: string, value: string, min: number, max: number) => {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return;
    void set(key, String(Math.max(min, Math.min(max, Math.round(parsed)))));
  };

  return (
    <>
      <PageHeader
        title={t("settings.title")}
        subtitle={t("settings.subtitle")}
        badge={<span className="pill pill--accent">M2</span>}
      />

      <div className="settings-grid">
        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="palette" size={20} />
            <h3>{t("settings.appearance")}</h3>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.theme")}</strong>
            </div>
            <Segmented
              value={theme}
              label={t("settings.theme")}
              options={[
                { value: "system", label: t("settings.themeSystem"), icon: "brightness_auto" },
                { value: "light", label: t("settings.themeLight"), icon: "light_mode" },
                { value: "dark", label: t("settings.themeDark"), icon: "dark_mode" },
              ]}
              onChange={(value) => void set("ui.theme", value)}
            />
          </div>

          <div className="settings-row settings-row--column">
            <div className="settings-row__text">
              <strong>{t("settings.seed")}</strong>
            </div>
            <div className="swatches">
              {SWATCHES.map((color) => (
                <button
                  key={color}
                  type="button"
                  className={`swatch${seed.toLowerCase() === color.toLowerCase() ? " is-active" : ""}`}
                  style={{ background: color }}
                  aria-label={color}
                  onClick={() => void set("ui.seed_color", color)}
                />
              ))}
              <label className="swatch swatch--custom" title={t("settings.seed")}>
                <Icon name="colorize" size={18} />
                <input
                  type="color"
                  value={seed}
                  onChange={(event) => void set("ui.seed_color", event.target.value)}
                />
              </label>
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.language")}</strong>
            </div>
            <select
              className="m3-select"
              value={language}
              onChange={(event) => void set("ui.language", event.target.value)}
            >
              <option value="ru">Русский</option>
              <option value="en">English</option>
            </select>
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="lan" size={20} />
            <h3>{t("settings.connection")}</h3>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.mode")}</strong>
            </div>
            <Segmented
              value={mode}
              label={t("settings.mode")}
              options={[
                { value: "proxy", label: t("home.proxy"), icon: "public" },
                { value: "tun", label: t("home.tun"), icon: "shield" },
              ]}
              onChange={(value) => void set("mode", value)}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.proxyPort")}</strong>
              <span>10808</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              min={1024}
              max={65535}
              value={settings["proxy.port"] ?? "10808"}
              onChange={(event) => saveNumber("proxy.port", event.target.value, 1024, 65535)}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.autoPort")}</strong>
              <span>{t("settings.autoPortHint")}</span>
            </div>
            <Switch
              checked={autoPort}
              label={t("settings.autoPort")}
              onChange={(checked) => void set("proxy.auto_port", checked ? "true" : "false")}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.activePort")}</strong>
              <span className="port-chip">127.0.0.1:{activePort}</span>
            </div>
            <Button
              variant="tonal"
              icon="autorenew"
              disabled={status.running}
              title={status.running ? t("settings.portLocked") : undefined}
              onClick={() => void pickPort()}
            >
              {t("settings.pickPort")}
            </Button>
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="speed" size={20} />
            <h3>{t("settings.ping")}</h3>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("home.autoPing")}</strong>
              <span>{pingEnabled ? t("home.pingOn") : t("home.pingOff")}</span>
            </div>
            <Switch
              checked={pingEnabled}
              label={t("home.autoPing")}
              onChange={(checked) => void set("ping.enabled", checked ? "true" : "false")}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.interval")}</strong>
              <span>5–1800</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              min={5}
              max={1800}
              value={settings["ping.interval_sec"] ?? "60"}
              onChange={(event) => saveNumber("ping.interval_sec", event.target.value, 5, 1800)}
            />
          </div>

          <Field label={t("settings.testUrl")} className="settings-field">
            <input
              className="m3-input"
              type="url"
              defaultValue={settings["ping.url"] ?? ""}
              onBlur={(event) => {
                void set("ping.url", event.target.value.trim());
                notify(t("snackbar.saved"));
              }}
            />
          </Field>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.concurrency")}</strong>
              <span>1–32</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              min={1}
              max={32}
              value={settings["ping.concurrency"] ?? "4"}
              onChange={(event) => saveNumber("ping.concurrency", event.target.value, 1, 32)}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.selectMode")}</strong>
            </div>
            <Segmented
              value={selectMode}
              label={t("settings.selectMode")}
              options={[
                { value: "auto", label: t("settings.selectAuto"), icon: "auto_awesome" },
                { value: "manual", label: t("settings.selectManual"), icon: "touch_app" },
              ]}
              onChange={(value) => void set("select.mode", value)}
            />
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="rocket_launch" size={20} />
            <h3>{t("settings.startup")}</h3>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.closeToTray")}</strong>
              <span>{t("settings.closeToTrayHint")}</span>
            </div>
            <Switch
              checked={settings["startup.close_to_tray"] !== "false"}
              label={t("settings.closeToTray")}
              onChange={(checked) => void set("startup.close_to_tray", checked ? "true" : "false")}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.autoconnect")}</strong>
              <span>{t("settings.autoconnectHint")}</span>
            </div>
            <Switch
              checked={settings["startup.autoconnect"] === "true"}
              label={t("settings.autoconnect")}
              onChange={(checked) => void set("startup.autoconnect", checked ? "true" : "false")}
            />
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="memory" size={20} />
            <h3>{t("settings.core")}</h3>
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{core?.available ? t("settings.coreReady") : t("settings.coreMissing")}</strong>
              <span className="path">{core?.xrayPath ?? "—"}</span>
            </div>
            <span className={`status-dot${core?.available ? " is-on" : ""}`} />
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.xrayVersion")}</strong>
              <span>{core?.xrayVersion ?? "—"}</span>
            </div>
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.assetsDir")}</strong>
              <span className="path">{core?.assetsDir ?? "—"}</span>
            </div>
            <Button variant="tonal" icon="folder_open" onClick={() => void openCoreDir()}>
              {t("settings.openCoreDir")}
            </Button>
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="description" size={20} />
            <h3>{t("settings.logs")}</h3>
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.logPath")}</strong>
              <span className="path">{info?.logDir ?? "~/.local/share/m3vpn/logs"}</span>
            </div>
            <Button variant="tonal" icon="folder_open" onClick={() => void openLogs()}>
              {t("settings.openLogs")}
            </Button>
          </div>
        </Card>

        <Card className="settings-card">
          <div className="card-heading">
            <Icon name="info" size={20} />
            <h3>{t("settings.about")}</h3>
          </div>
          <dl className="facts facts--two">
            <div>
              <dt>{t("settings.version")}</dt>
              <dd>v{info?.version ?? "0.1.0"}</dd>
            </div>
            <div>
              <dt>{t("settings.tauri")}</dt>
              <dd>{info?.tauriVersion ?? "2.x"}</dd>
            </div>
            <div>
              <dt>{t("settings.platform")}</dt>
              <dd>{info?.platform ?? "linux x86_64"}</dd>
            </div>
            <div>
              <dt>{t("settings.dataDir")}</dt>
              <dd className="path">{info?.dataDir ?? "~/.local/share/m3vpn"}</dd>
            </div>
          </dl>
        </Card>
      </div>
    </>
  );
}
