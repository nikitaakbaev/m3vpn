import { useEffect, useRef, useState } from "react";
import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import { formatDate, formatBytes, parseUserinfo } from "../../lib/format";
import type { ImportSummary } from "../../lib/tauri";

export function SubscriptionsPage() {
  const t = useT();
  const profiles = useAppStore((state) => state.profiles);
  const importing = useAppStore((state) => state.importing);
  const importSubscription = useAppStore((state) => state.importSubscription);
  const importAwg = useAppStore((state) => state.importAwg);
  const deleteProfile = useAppStore((state) => state.deleteProfile);
  const refreshProfile = useAppStore((state) => state.refreshProfile);
  const refreshingId = useAppStore((state) => state.refreshingId);
  const notify = useAppStore((state) => state.notify);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [lastImport, setLastImport] = useState<ImportSummary | null>(null);

  useEffect(() => {
    if (new URLSearchParams(window.location.search).get("add") === "1") {
      dialogRef.current?.showModal();
    }
  }, []);

  const openDialog = () => {
    setError(null);
    dialogRef.current?.showModal();
  };

  const closeDialog = () => {
    if (importing) return;
    dialogRef.current?.close();
    setUrl("");
    setName("");
    setError(null);
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    const text = url.trim();
    const isAwg = text.includes("[Interface]") || /^(amneziawg|awg):\/\//i.test(text);
    if (!isAwg && !/^https?:\/\//.test(text)) {
      setError(t("subs.invalidUrl"));
      return;
    }
    try {
      const summary = isAwg
        ? await importAwg(text, name.trim() || undefined)
        : await importSubscription(text, name.trim() || undefined);
      setLastImport(summary);
      dialogRef.current?.close();
      setUrl("");
      setName("");
      notify(
        `${t("subs.imported")}: ${summary.servers}` +
          (summary.skipped ? ` · ${t("subs.skipped")}: ${summary.skipped}` : ""),
      );
    } catch (caught) {
      setError(String(caught));
    }
  };

  return (
    <>
      <PageHeader
        title={t("subs.title")}
        subtitle={t("subs.subtitle")}
        badge={<span className="pill pill--accent">M5</span>}
        actions={
          <Button icon="add_link" onClick={openDialog}>
            {t("subs.add")}
          </Button>
        }
      />

      {lastImport && (lastImport.skipped > 0 || lastImport.notes.length > 0) ? (
        <Card className="notice-card">
          <div className="notice-card__icon">
            <Icon name="info" size={20} filled />
          </div>
          <div>
            <strong>
              {t("subs.imported")}: {lastImport.servers}
              {lastImport.skipped ? ` · ${t("subs.skipped")}: ${lastImport.skipped}` : ""}
            </strong>
            {lastImport.notes.length > 0 ? <p>{lastImport.notes.join(" · ")}</p> : null}
          </div>
          <button type="button" className="icon-button" onClick={() => setLastImport(null)} aria-label={t("common.close")}>
            <Icon name="close" size={18} />
          </button>
        </Card>
      ) : null}

      {profiles.length === 0 ? (
        <Card className="empty-card">
          <EmptyState
            icon="playlist_add"
            title={t("subs.emptyTitle")}
            text={t("subs.emptyText")}
            action={
              <Button icon="add_link" onClick={openDialog}>
                {t("subs.add")}
              </Button>
            }
          />
        </Card>
      ) : (
        <div className="profile-grid">
          {profiles.map((profile) => {
            const info = parseUserinfo(profile.userinfo);
            const used = (info.upload ?? 0) + (info.download ?? 0);
            const total = info.total ?? 0;
            const percent = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
            return (
              <Card key={profile.id} className="profile-card">
                <div className="profile-card__head">
                  <div className="profile-card__title">
                    <span className="profile-card__avatar">
                      <Icon name="rss_feed" size={20} />
                    </span>
                    <div>
                      <h3>{profile.name}</h3>
                      <span className="muted">
                        {profile.serverCount} {t("home.serversShort")} · {t("subs.every")}{" "}
                        {profile.updateIntervalHours} {t("subs.hours")}
                      </span>
                    </div>
                  </div>
                  <div className="profile-card__actions">
                    {profile.urlPresent ? (
                      <button
                        type="button"
                        className="icon-button"
                        aria-label={t("subs.refresh")}
                        title={t("subs.refresh")}
                        disabled={refreshingId === profile.id}
                        onClick={() => void refreshProfile(profile.id)}
                      >
                        <Icon
                          name={refreshingId === profile.id ? "progress_activity" : "refresh"}
                          size={19}
                        />
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="icon-button icon-button--danger"
                      aria-label={t("subs.delete")}
                      onClick={() => {
                        if (window.confirm(`${t("subs.delete")}: ${profile.name}?`)) {
                          void deleteProfile(profile.id);
                        }
                      }}
                    >
                      <Icon name="delete" size={19} />
                    </button>
                  </div>
                </div>

                {profile.userinfo ? (
                  <div className="profile-card__traffic">
                    <div className="profile-card__traffic-row">
                      <strong>{formatBytes(used)}</strong>
                      <span className="muted">/ {formatBytes(total)}</span>
                    </div>
                    <div className="progress">
                      <span style={{ width: `${percent}%` }} />
                    </div>
                    <div className="profile-card__meta">
                      <span>{t("subs.expires")}: {formatDate(info.expire)}</span>
                      <span>{percent}%</span>
                    </div>
                  </div>
                ) : (
                  <p className="muted">{t("subs.noTraffic")}</p>
                )}

                <div className="profile-card__footer">
                  <span className="pill pill--muted">
                    <Icon name={profile.hideUrl ? "visibility_off" : "link"} size={14} />
                    {profile.hideUrl ? t("subs.hiddenUrl") : t("subs.fullXray")}
                  </span>
                  <span className="muted">
                    {t("subs.updated")}: {formatDate(profile.lastRefreshAt ?? profile.createdAt)}
                  </span>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Card className="formats-card">
        <div className="card-heading">
          <Icon name="verified" size={20} />
          <h3>{t("subs.formatsTitle")}</h3>
        </div>
        <p>{t("subs.formatsText")}</p>
        <div className="chip-row">
          <span className="chip">Full Xray JSON</span>
          <span className="chip">VLESS · Reality</span>
          <span className="chip">AmneziaWG</span>
          <span className="chip">Hysteria2</span>
          <span className="chip">WireGuard</span>
          <span className="chip">SOCKS5</span>
          <span className="chip">HTTP</span>
        </div>
      </Card>

      <dialog
        ref={dialogRef}
        className="m3-dialog m3-dialog--form"
        aria-labelledby="subs-add-title"
        onCancel={(event) => {
          if (importing) {
            event.preventDefault();
            return;
          }
          closeDialog();
        }}
      >
        <form onSubmit={submit}>
          <div className="m3-dialog__head">
            <span className="m3-dialog__icon m3-dialog__icon--accent">
              <Icon name="add_link" size={22} />
            </span>
            <div>
              <h2 id="subs-add-title">{t("subs.addTitle")}</h2>
              <p>{t("subs.addHint")}</p>
            </div>
          </div>

          <label className="m3-field">
            <span className="m3-field__label">{t("subs.urlLabel")}</span>
            <textarea
              className="m3-input m3-textarea"
              autoFocus
              rows={4}
              placeholder="https://provider.example/sub/... или [Interface] ..."
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              disabled={importing}
            />
            <span className="m3-field__hint">{t("subs.awgHint")}</span>
          </label>

          <label className="m3-field">
            <span className="m3-field__label">{t("subs.nameLabel")}</span>
            <input
              className="m3-input"
              type="text"
              placeholder={t("subs.namePlaceholder")}
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={importing}
            />
          </label>

          {error ? (
            <div className="form-error" role="alert">
              <Icon name="error" size={18} filled />
              <span>{error}</span>
            </div>
          ) : null}

          <div className="m3-dialog__actions">
            <Button variant="text" onClick={closeDialog} disabled={importing}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" icon={importing ? "progress_activity" : "download"} disabled={importing}>
              {importing ? t("subs.importing") : t("subs.import")}
            </Button>
          </div>
        </form>
      </dialog>
    </>
  );
}
