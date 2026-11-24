import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";

export function SubscriptionsPage() {
  const t = useT();
  const notify = useAppStore((state) => state.notify);

  return (
    <>
      <PageHeader
        title={t("subs.title")}
        subtitle={t("subs.subtitle")}
        badge={<span className="pill pill--tonal">M3</span>}
        actions={
          <Button icon="add_link" onClick={() => notify(t("subs.m3"))}>
            {t("subs.add")}
          </Button>
        }
      />

      <Card className="empty-card">
        <EmptyState
          icon="playlist_add"
          title={t("subs.emptyTitle")}
          text={t("subs.emptyText")}
          action={
            <Button variant="tonal" icon="schedule" onClick={() => notify(t("subs.m3"))}>
              {t("subs.m3")}
            </Button>
          }
        />
      </Card>

      <Card className="formats-card">
        <div className="card-heading">
          <Icon name="verified" size={20} />
          <h3>{t("subs.formatsTitle")}</h3>
        </div>
        <p>{t("subs.formatsText")}</p>
        <div className="chip-row">
          <span className="chip">VLESS · Reality</span>
          <span className="chip">AmneziaWG</span>
          <span className="chip">Hysteria2</span>
          <span className="chip">WireGuard</span>
          <span className="chip">SOCKS5</span>
          <span className="chip">HTTP</span>
          <span className="chip">Full Xray JSON</span>
        </div>
      </Card>
    </>
  );
}
