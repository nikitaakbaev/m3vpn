import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { Field } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import type { ThemeMode } from "../../theme";

const SWATCHES = ["#22C55E", "#2563EB", "#7C3AED", "#EA580C", "#0F766E", "#CA8A04"];

const PING_PRESETS = [
  { key: "google", url: "https://www.gstatic.com/generate_204" },
  { key: "cloudflare", url: "https://cp.cloudflare.com/generate_204" },
  { key: "apple", url: "https://captive.apple.com/hotspot-detect.html" },
] as const;

export function SettingsPage() {
  const t = useT();
  const settings = useAppStore((state) => state.settings);
  const info = useAppStore((state) => state.info);
  const set = useAppStore((state) => state.set);
  const pickPort = useAppStore((state) => state.pickPort);
  const openLogs = useAppStore((state) => state.openLogs);
  const openCoreDir = useAppStore((state) => state.openCoreDir);
  const core = useAppStore((state) => state.core);
  const updateInfo = useAppStore((state) => state.updateInfo);
  const updateBusy = useAppStore((state) => state.updateBusy);
  const updateError = useAppStore((state) => state.updateError);
  const checkUpdates = useAppStore((state) => state.checkUpdates);
  const openUpdateArtifact = useAppStore((state) => state.openUpdateArtifact);
  const isConnected = useAppStore((state) => state.status.running);
  const notify = useAppStore((state) => state.notify);

  const theme = (settings["ui.theme"] ?? "dark") as ThemeMode;
  const seed = settings["ui.seed_color"] ?? "#22C55E";
  const language = settings["ui.language"] === "en" ? "en" : "ru";
  const mode = settings["mode"] === "tun" ? "tun" : "proxy";
  const helperAvailable = info?.helperAvailable ?? false;
  const autoPort = settings["proxy.auto_port"] !== "false";
  const activePort = settings["proxy.active_port"] ?? String(info?.proxyPort ?? 10808);
  const pingEnabled = settings["ping.enabled"] !== "false";
  const selectMode = settings["select.mode"] === "manual" ? "manual" : "auto";
  const autoTolerance = Math.round(
    Number(settings["select.tolerance_ratio"] ?? "0.3") * 100,
  );
  const autoWins = Number(settings["select.wins_required"] ?? "2");
  const updateRepo = settings["update.github_repo"] ?? "";
  const updateAutocheck = settings["update.autocheck"] !== "false";
  const currentVersion = info?.version ?? "0.1.0";
  const serverSort = settings["servers.sort"] ?? "group";
  const pingProtocol = settings["ping.protocol"] ?? "fast_http";
  const pingDisplay = settings["ping.display"] ?? "both";
  const pingUrl = settings["ping.url"] ?? "https://www.gstatic.com/generate_204";
  const pingTimeoutSec = Math.max(
    1,
    Math.round(Number(settings["ping.timeout_ms"] ?? "3000") / 1000),
  );
  const pingHintKey =
    pingProtocol === "tcp"
      ? "settings.pingTcpHint"
      : pingProtocol === "fast_http"
        ? "settings.pingFastHttpHint"
        : "settings.pingProxyHint";

  const changePingTimeout = (delta: number) => {
    const next = Math.max(1, Math.min(10, pingTimeoutSec + delta));
    void set("ping.timeout_ms", String(next * 1000));
  };

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
        badge={<span className="pill pill--accent">M9</span>}
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
              aria-label={t("settings.language")}
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
                { value: "tun", label: t("home.tun"), icon: "shield", disabled: !helperAvailable },
              ]}
              onChange={(value) => void set("mode", value)}
            />
          </div>
          <p className="hint">{helperAvailable ? t("settings.tunReadyHint") : t("settings.tunMissingHint")}</p>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.proxyPort")}</strong>
              <span>10808</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              aria-label={t("settings.proxyPort")}
              min={1024}
              max={65535}
              value={settings["proxy.port"] ?? "10808"}
              onChange={(event) => saveNumber("proxy.port", event.target.value, 1024, 65535)}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.killSwitch")}</strong>
              <span>{t("settings.killSwitchHint")}</span>
            </div>
            <Switch
              checked={settings["tun.kill_switch"] === "true"}
              disabled={!helperAvailable}
              label={t("settings.killSwitch")}
              onChange={(checked) => void set("tun.kill_switch", checked ? "true" : "false")}
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
              disabled={isConnected}
              title={isConnected ? t("settings.portLocked") : undefined}
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

          <div className="settings-row settings-row--column">
            <div className="settings-row__text">
              <strong>{t("settings.pingProtocol")}</strong>
              <span>{t(pingHintKey)}</span>
            </div>
            <div className="settings-segmented">
              <Segmented
                value={pingProtocol}
                label={t("settings.pingProtocol")}
                options={[
                  { value: "fast_http", label: t("settings.pingFastHttp") },
                  { value: "tcp", label: t("settings.pingTcp") },
                  { value: "proxy_get", label: t("settings.pingGet") },
                  { value: "proxy_head", label: t("settings.pingHead") },
                ]}
                onChange={(value) => void set("ping.protocol", value)}
              />
            </div>
          </div>

          <div className="settings-row settings-row--column">
            <div className="settings-row__text">
              <strong>{t("settings.pingDisplay")}</strong>
            </div>
            <div className="settings-segmented">
              <Segmented
                value={pingDisplay}
                label={t("settings.pingDisplay")}
                options={[
                  { value: "numbers", label: t("settings.pingNumbers") },
                  { value: "scale", label: t("settings.pingScale") },
                  { value: "both", label: t("settings.pingBoth") },
                  { value: "dots", label: t("settings.pingDots") },
                ]}
                onChange={(value) => void set("ping.display", value)}
              />
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.interval")}</strong>
              <span>5–1800</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              aria-label={t("settings.interval")}
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
              value={pingUrl}
              onChange={(event) => void set("ping.url", event.target.value)}
              onBlur={(event) => {
                void set("ping.url", event.target.value.trim());
                notify(t("snackbar.saved"));
              }}
            />
          </Field>

          <div className="ping-presets" role="group" aria-label={t("settings.pingPresets")}>
            {PING_PRESETS.map((preset) => {
              const active = pingUrl === preset.url;
              return (
                <button
                  key={preset.key}
                  type="button"
                  className={`ping-preset${active ? " is-active" : ""}`}
                  onClick={() => void set("ping.url", preset.url)}
                >
                  <span>{t(`settings.pingPreset.${preset.key}`)}</span>
                  {active ? <Icon name="check" size={17} /> : null}
                </button>
              );
            })}
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.pingTimeout")}</strong>
              <span>{t("settings.pingTimeoutHint")}</span>
            </div>
            <div className="stepper">
              <button type="button" aria-label="−" onClick={() => changePingTimeout(-1)}>
                −
              </button>
              <span>
                {pingTimeoutSec} {t("settings.secondsShort")}
              </span>
              <button type="button" aria-label="+" onClick={() => changePingTimeout(1)}>
                +
              </button>
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.concurrency")}</strong>
              <span>{t("settings.concurrencyHint")}</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              aria-label={t("settings.concurrency")}
              min={1}
              max={32}
              value={settings["ping.concurrency"] ?? "8"}
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
          <div className="settings-row settings-row--column">
            <div className="settings-row__text">
              <strong>{t("settings.serverSort")}</strong>
              <span>{t("settings.serverSortHint")}</span>
            </div>
            <div className="settings-segmented">
              <Segmented
                value={serverSort}
                label={t("settings.serverSort")}
                options={[
                  { value: "group", label: t("servers.sortGroup"), icon: "workspaces" },
                  { value: "ping", label: t("servers.sortPing"), icon: "speed" },
                  { value: "name", label: t("servers.sortName"), icon: "sort_by_alpha" },
                ]}
                onChange={(value) => void set("servers.sort", value)}
              />
            </div>
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.autoTolerance")}</strong>
              <span>{t("settings.autoToleranceHint")}</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              aria-label={t("settings.autoTolerance")}
              min={1}
              max={80}
              value={autoTolerance}
              onChange={(event) => {
                const percent = Number(event.target.value);
                if (!Number.isFinite(percent)) return;
                const clamped = Math.max(1, Math.min(80, Math.round(percent)));
                void set("select.tolerance_ratio", (clamped / 100).toFixed(2));
              }}
            />
          </div>
          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.autoWins")}</strong>
              <span>{t("settings.autoWinsHint")}</span>
            </div>
            <input
              className="m3-input m3-input--short"
              type="number"
              aria-label={t("settings.autoWins")}
              min={1}
              max={5}
              value={autoWins}
              onChange={(event) => {
                const wins = Number(event.target.value);
                if (!Number.isFinite(wins)) return;
                void set("select.wins_required", String(Math.max(1, Math.min(5, Math.round(wins)))));
              }}
            />
          </div>
          <p className="hint">{t("settings.selectHint")}</p>
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
            <Icon name="system_update" size={20} />
            <h3>{t("settings.updates")}</h3>
          </div>

          <div className="settings-row settings-row--column">
            <div className="settings-row__text">
              <strong>{t("settings.updateRepo")}</strong>
              <span>{t("settings.updateRepoHint")}</span>
            </div>
            <input
              className="m3-input"
              type="text"
              value={updateRepo}
              placeholder="owner/m3vpn"
              onChange={(event) => void set("update.github_repo", event.target.value)}
              onBlur={(event) => void set("update.github_repo", event.target.value.trim())}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.updateAutocheck")}</strong>
              <span>{t("settings.updateAutocheckHint")}</span>
            </div>
            <Switch
              checked={updateAutocheck}
              label={t("settings.updateAutocheck")}
              onChange={(checked) => void set("update.autocheck", checked ? "true" : "false")}
            />
          </div>

          <div className="settings-row">
            <div className="settings-row__text">
              <strong>{t("settings.updateCurrent")}: {currentVersion}</strong>
              <span>
                {updateInfo?.latestVersion
                  ? `${t("settings.updateLatest")}: ${updateInfo.latestVersion}`
                  : t("settings.updateRepoMissing")}
              </span>
            </div>
            <Button
              variant="tonal"
              icon={updateBusy ? "progress_activity" : "sync"}
              disabled={updateBusy || !updateRepo.trim()}
              onClick={() => void checkUpdates()}
            >
              {updateBusy ? t("settings.updateChecking") : t("settings.updateCheck")}
            </Button>
          </div>

          {updateError ? (
            <div className="form-error" role="alert">
              <Icon name="error" size={18} filled />
              <span>{updateError}</span>
            </div>
          ) : null}

          {updateInfo?.configured ? (
            <div className={`update-status${updateInfo.available ? " is-available" : ""}`}>
              <div className="update-status__head">
                <Icon
                  name={updateInfo.available ? "new_releases" : "verified"}
                  size={20}
                  filled
                />
                <div>
                  <strong>
                    {updateInfo.available
                      ? t("settings.updateAvailable")
                      : t("settings.updateNoUpdates")}
                  </strong>
                  <span>
                    {t("settings.updateLatest")}: {updateInfo.latestVersion ?? "—"}
                    {updateInfo.publishedAt
                      ? ` · ${t("settings.updatePublished")}: ${updateInfo.publishedAt}`
                      : ""}
                  </span>
                </div>
              </div>
              {updateInfo.notes ? <p>{updateInfo.notes}</p> : null}
              {updateInfo.artifacts.length > 0 || updateInfo.releaseUrl ? (
                <div className="update-artifacts">
                  {updateInfo.releaseUrl ? (
                    <Button
                      variant="text"
                      icon="open_in_new"
                      onClick={() => void openUpdateArtifact(updateInfo.releaseUrl ?? "")}
                    >
                      {t("settings.updateOpenRelease")}
                    </Button>
                  ) : null}
                  {updateInfo.artifacts.map((artifact) => (
                    <Button
                      key={artifact.kind}
                      variant="outlined"
                      icon="download"
                      title={`sha256: ${artifact.sha256} · ${artifact.size} B`}
                      onClick={() => void openUpdateArtifact(artifact.url)}
                    >
                      {artifact.kind === "arch"
                        ? t("settings.updateArch")
                        : artifact.kind === "portable"
                          ? t("settings.updatePortable")
                          : artifact.kind}
                    </Button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}
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
              <strong>sing-box</strong>
              <span className="path">{core?.singboxPath ?? "—"}</span>
              <span>{core?.singboxVersion ?? "—"}</span>
            </div>
            <span className={`status-dot${core?.singboxAvailable ? " is-on" : ""}`} />
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
