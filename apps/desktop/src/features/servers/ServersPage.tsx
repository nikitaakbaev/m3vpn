import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useT } from "../../store/useAppStore";
import type { Page } from "../../components/NavRail";

interface ServersPageProps {
  onNavigate: (page: Page) => void;
}

export function ServersPage({ onNavigate }: ServersPageProps) {
  const t = useT();

  return (
    <>
      <PageHeader
        title={t("servers.title")}
        subtitle={t("servers.subtitle")}
        badge={<span className="pill pill--tonal">M2</span>}
        actions={
          <Button icon="add" onClick={() => onNavigate("subscriptions")}>
            {t("servers.add")}
          </Button>
        }
      />

      <Card className="toolbar-card">
        <label className="search">
          <Icon name="search" size={20} />
          <input placeholder={t("servers.search")} disabled />
        </label>
        <div className="chip-row">
          <span className="chip">Auto</span>
          <span className="chip">Ping 60s</span>
          <span className="chip">Reality</span>
          <span className="chip">AmneziaWG</span>
        </div>
      </Card>

      <Card className="empty-card">
        <EmptyState
          icon="dns"
          title={t("servers.emptyTitle")}
          text={t("servers.emptyText")}
          action={
            <Button icon="playlist_add" onClick={() => onNavigate("subscriptions")}>
              {t("servers.toSubscriptions")}
            </Button>
          }
        />
      </Card>
    </>
  );
}
