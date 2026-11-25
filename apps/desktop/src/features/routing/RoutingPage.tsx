import { useEffect, useRef, useState } from "react";
import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import { formatBytes, formatDate } from "../../lib/format";

export function RoutingPage() {
  const t = useT();
  const profiles = useAppStore((state) => state.routingProfiles);
  const busy = useAppStore((state) => state.routingBusy);
  const loadRouting = useAppStore((state) => state.loadRouting);
  const importRouting = useAppStore((state) => state.importRouting);
  const saveRouting = useAppStore((state) => state.saveRouting);
  const deleteRouting = useAppStore((state) => state.deleteRouting);
  const activateRouting = useAppStore((state) => state.activateRouting);
  const refreshGeo = useAppStore((state) => state.refreshGeo);
  const ruleSets = useAppStore((state) => state.routingRuleSets);
  const settings = useAppStore((state) => state.settings);
  const setSetting = useAppStore((state) => state.set);

  const importRef = useRef<HTMLDialogElement>(null);
  const editRef = useRef<HTMLDialogElement>(null);
  const [importText, setImportText] = useState("");
  const [importName, setImportName] = useState("");
  const [editId, setEditId] = useState<string | null>(null);
  const [editJson, setEditJson] = useState("");
  const [editName, setEditName] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadRouting();
  }, [loadRouting]);

  const active = profiles.find((profile) => profile.active) ?? null;
  const geoHours = Number(settings["routing.geo_update_hours"] ?? "24");
  const geoSource = settings["routing.geo_source"] === "ru" ? "ru" : "global";

  const openImport = () => {
    setError(null);
    setImportText("");
    setImportName("");
    importRef.current?.showModal();
  };

  const submitImport = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    if (!importText.trim()) {
      setError(t("routing.emptyJson"));
      return;
    }
    const profile = await importRouting(importText.trim(), importName.trim() || undefined);
    if (profile) {
      importRef.current?.close();
      setImportText("");
      setImportName("");
    }
  };

  const openEdit = (id: string) => {
    const profile = profiles.find((item) => item.id === id);
    if (!profile) return;
    setEditId(id);
    setEditName(profile.name);
    setEditJson(profile.profileJson);
    setError(null);
    editRef.current?.showModal();
  };

  const submitEdit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    if (!editJson.trim()) {
      setError(t("routing.emptyJson"));
      return;
    }
    const saved = await saveRouting(editId, editJson.trim(), editName.trim() || undefined);
    if (saved) {
      editRef.current?.close();
    }
  };

  return (
    <>
      <PageHeader
        title={t("routing.title")}
        subtitle={t("routing.subtitle")}
        badge={<span className="pill pill--accent">M9</span>}
        actions={
          <>
            <Button variant="text" icon="refresh" onClick={() => void loadRouting()}>
              {t("common.refresh")}
            </Button>
            <Button icon="add" onClick={openImport}>
              {t("routing.add")}
            </Button>
          </>
        }
      />

      <Card className="notice-card notice-card--geo">
        <div className="notice-card__icon">
          <Icon name="route" size={20} filled />
        </div>
        <div className="geo-card__body">
          <strong>{t("routing.geoTitle")}</strong>
          <p>{t("routing.geoText")}</p>
          {ruleSets.length > 0 && (
            <div className="rule-set-chips" aria-label={t("routing.ruleSets")}>
              <span className="rule-set-chips__label">{t("routing.ruleSets")}</span>
              {ruleSets.map((set) => (
                <span
                  key={set.tag}
                  className="chip"
                  title={`${formatBytes(set.bytes)} · ${formatDate(set.modifiedAt ?? undefined)}`}
                >
                  {set.tag}
                </span>
              ))}
            </div>
          )}
        </div>
        <div className="geo-controls">
          <label className="geo-controls__interval">
            <span>{t("routing.geoSource")}</span>
            <select
              className="m3-input m3-input--short"
              value={geoSource}
              aria-label={t("routing.geoSource")}
              onChange={(event) => void setSetting("routing.geo_source", event.target.value)}
            >
              <option value="global">{t("routing.geoGlobal")}</option>
              <option value="ru">{t("routing.geoRussia")}</option>
            </select>
          </label>
          <label className="geo-controls__interval">
            <span>{t("routing.geoEvery")}</span>
            <select
              className="m3-input m3-input--short"
              value={String(geoHours)}
              aria-label={t("routing.geoEvery")}
              onChange={(event) => void setSetting("routing.geo_update_hours", event.target.value)}
            >
              <option value="0">{t("routing.geoOff")}</option>
              <option value="12">12 {t("routing.hours")}</option>
              <option value="24">24 {t("routing.hours")}</option>
              <option value="48">48 {t("routing.hours")}</option>
              <option value="168">7 {t("routing.days")}</option>
            </select>
          </label>
          <Button
            variant="tonal"
            icon="cloud_download"
            disabled={busy || !active}
            onClick={() => void refreshGeo(active?.id)}
          >
            {busy ? t("routing.geoUpdating") : t("routing.geoRefresh")}
          </Button>
        </div>
      </Card>

      {profiles.length === 0 ? (
        <Card className="empty-card">
          <EmptyState
            icon="route"
            title={t("routing.emptyTitle")}
            text={t("routing.emptyText")}
            action={
              <Button icon="add" onClick={openImport}>
                {t("routing.add")}
              </Button>
            }
          />
        </Card>
      ) : (
        <div className="profile-grid">
          {profiles.map((profile) => (
            <Card key={profile.id} className="profile-card">
              <div className="profile-card__head">
                <div className="profile-card__title">
                  <span className="profile-card__avatar">
                    <Icon name="route" size={20} />
                  </span>
                  <div>
                    <h2>{profile.name}</h2>
                    <span className="muted">
                      {t("routing.lastUpdated")}: {formatDate(profile.lastUpdated ?? undefined)}
                    </span>
                  </div>
                </div>
                {profile.active ? (
                  <span className="pill pill--success">
                    <Icon name="check_circle" size={14} filled /> {t("routing.active")}
                  </span>
                ) : null}
              </div>

              <div className="routing-meta">
                <span className="chip">
                  <Icon name="travel_explore" size={14} />
                  {t("routing.geoLastCheck")}: {formatDate(profile.geoLastCheckAt ?? undefined)}
                </span>
                {profile.sourceUrl ? (
                  <span className="chip" title={profile.sourceUrl}>
                    <Icon name="link" size={14} /> {t("routing.source")}
                  </span>
                ) : null}
              </div>

              <div className="profile-card__footer routing-actions">
                <div className="routing-actions__buttons">
                  {!profile.active ? (
                    <Button variant="tonal" icon="check" onClick={() => void activateRouting(profile.id)}>
                      {t("routing.activate")}
                    </Button>
                  ) : null}
                  <Button variant="outlined" icon="edit" onClick={() => openEdit(profile.id)}>
                    {t("routing.edit")}
                  </Button>
                  <Button
                    variant="text"
                    icon="cloud_download"
                    disabled={busy}
                    onClick={() => void refreshGeo(profile.id)}
                  >
                    {t("routing.geoRefresh")}
                  </Button>
                </div>
                <button
                  type="button"
                  className="icon-button icon-button--danger"
                  aria-label={t("routing.delete")}
                  onClick={() => {
                    if (window.confirm(`${t("routing.delete")}: ${profile.name}?`)) {
                      void deleteRouting(profile.id);
                    }
                  }}
                >
                  <Icon name="delete" size={19} />
                </button>
              </div>
            </Card>
          ))}
        </div>
      )}

      <Card className="formats-card">
        <div className="card-heading">
          <Icon name="verified" size={20} />
          <h2>{t("routing.formatsTitle")}</h2>
        </div>
        <p>{t("routing.formatsText")}</p>
        <div className="chip-row">
          <span className="chip">GlobalProxy</span>
          <span className="chip">Direct / Proxy / Block</span>
          <span className="chip">geoip / geosite</span>
          <span className="chip">DoH / DoU</span>
          <span className="chip">FakeDNS (sing-box)</span>
          <span className="chip">DomainStrategy</span>
          <span className="chip">m3vpn://routing</span>
        </div>
      </Card>

      <dialog
        ref={importRef}
        className="m3-dialog m3-dialog--form"
        aria-labelledby="routing-import-title"
        onCancel={(event) => {
          if (busy) {
            event.preventDefault();
            return;
          }
          importRef.current?.close();
        }}
      >
        <form onSubmit={submitImport}>
          <div className="m3-dialog__head">
            <span className="m3-dialog__icon m3-dialog__icon--accent">
              <Icon name="add_road" size={22} />
            </span>
            <div>
              <h2 id="routing-import-title">{t("routing.importTitle")}</h2>
              <p>{t("routing.importHint")}</p>
            </div>
          </div>
          <label className="m3-field">
            <span className="m3-field__label">{t("routing.importLabel")}</span>
            <textarea
              className="m3-input m3-textarea"
              rows={7}
              value={importText}
              onChange={(event) => setImportText(event.target.value)}
              placeholder={t("routing.importPlaceholder")}
            />
          </label>
          <label className="m3-field">
            <span className="m3-field__label">{t("subs.nameLabel")}</span>
            <input
              className="m3-input"
              value={importName}
              onChange={(event) => setImportName(event.target.value)}
              placeholder={t("subs.namePlaceholder")}
            />
          </label>
          {error ? (
            <div className="form-error" role="alert">
              <Icon name="error" size={18} filled />
              <span>{error}</span>
            </div>
          ) : null}
          <div className="m3-dialog__actions">
            <Button variant="text" onClick={() => importRef.current?.close()} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" icon="download" disabled={busy}>
              {busy ? t("routing.importing") : t("routing.importAction")}
            </Button>
          </div>
        </form>
      </dialog>

      <dialog
        ref={editRef}
        className="m3-dialog m3-dialog--form"
        aria-labelledby="routing-edit-title"
        onCancel={(event) => {
          if (busy) {
            event.preventDefault();
            return;
          }
          editRef.current?.close();
        }}
      >
        <form onSubmit={submitEdit}>
          <div className="m3-dialog__head">
            <span className="m3-dialog__icon m3-dialog__icon--accent">
              <Icon name="edit" size={22} />
            </span>
            <div>
              <h2 id="routing-edit-title">{t("routing.editTitle")}</h2>
              <p>{t("routing.editHint")}</p>
            </div>
          </div>
          <label className="m3-field">
            <span className="m3-field__label">{t("subs.nameLabel")}</span>
            <input
              className="m3-input"
              value={editName}
              onChange={(event) => setEditName(event.target.value)}
            />
          </label>
          <label className="m3-field">
            <span className="m3-field__label">{t("routing.jsonLabel")}</span>
            <textarea
              className="m3-input m3-textarea m3-textarea--tall"
              rows={16}
              value={editJson}
              onChange={(event) => setEditJson(event.target.value)}
            />
          </label>
          {error ? (
            <div className="form-error" role="alert">
              <Icon name="error" size={18} filled />
              <span>{error}</span>
            </div>
          ) : null}
          <div className="m3-dialog__actions">
            <Button variant="text" onClick={() => editRef.current?.close()} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" icon="save" disabled={busy}>
              {t("routing.save")}
            </Button>
          </div>
        </form>
      </dialog>
    </>
  );
}
