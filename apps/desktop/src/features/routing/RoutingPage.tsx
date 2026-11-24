import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";

export function RoutingPage() {
  const t = useT();
  const notify = useAppStore((state) => state.notify);

  return (
    <>
      <PageHeader
        title={t("routing.title")}
        subtitle={t("routing.subtitle")}
        badge={<span className="pill pill--tonal">M7</span>}
        actions={
          <Button icon="add" onClick={() => notify(t("common.soon"))}>
            {t("routing.add")}
          </Button>
        }
      />

      <Card className="empty-card">
        <EmptyState
          icon="route"
          title={t("routing.emptyTitle")}
          text={t("routing.emptyText")}
          action={
            <Button variant="tonal" icon="schedule" onClick={() => notify(t("common.soon"))}>
              {t("common.soon")}
            </Button>
          }
        />
      </Card>

      <Card className="formats-card">
        <div className="card-heading">
          <Icon name="public" size={20} />
          <h3>{t("routing.geoTitle")}</h3>
        </div>
        <p>{t("routing.geoText")}</p>
        <div className="chip-row">
          <span className="chip">geoip.dat</span>
          <span className="chip">geosite.dat</span>
          <span className="chip">DoH / DoU</span>
          <span className="chip">FakeDNS</span>
          <span className="chip">Direct / Proxy / Block</span>
        </div>
      </Card>
    </>
  );
}
