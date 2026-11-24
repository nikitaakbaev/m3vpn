import { useMemo, useState } from "react";
import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import type { Page } from "../../components/NavRail";

interface ServersPageProps {
  onNavigate: (page: Page) => void;
}

export function ServersPage({ onNavigate }: ServersPageProps) {
  const t = useT();
  const servers = useAppStore((state) => state.servers);
  const profiles = useAppStore((state) => state.profiles);
  const status = useAppStore((state) => state.status);
  const selectedServerId = useAppStore((state) => state.selectedServerId);
  const selectServer = useAppStore((state) => state.selectServer);
  const connect = useAppStore((state) => state.connect);
  const disconnect = useAppStore((state) => state.disconnect);
  const refreshData = useAppStore((state) => state.refreshData);
  const busy = useAppStore((state) => state.busy);
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return servers;
    return servers.filter((server) =>
      `${server.name} ${server.host ?? ""} ${server.profileName} ${server.protocol}`
        .toLowerCase()
        .includes(needle),
    );
  }, [servers, query]);

  const groups = useMemo(() => {
    const map = new Map<string, typeof filtered>();
    for (const server of filtered) {
      const list = map.get(server.profileName) ?? [];
      list.push(server);
      map.set(server.profileName, list);
    }
    return [...map.entries()];
  }, [filtered]);

  return (
    <>
      <PageHeader
        title={t("servers.title")}
        subtitle={t("servers.subtitle")}
        badge={<span className="pill pill--accent">M2</span>}
        actions={
          <>
            <Button variant="text" icon="refresh" onClick={() => void refreshData()}>
              {t("common.refresh")}
            </Button>
            <Button icon="add_link" onClick={() => onNavigate("subscriptions")}>
              {t("servers.add")}
            </Button>
          </>
        }
      />

      <Card className="toolbar-card">
        <label className="search">
          <Icon name="search" size={19} />
          <input
            placeholder={t("servers.search")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          {query ? (
            <button type="button" className="search__clear" onClick={() => setQuery("")} aria-label={t("common.clear")}>
              <Icon name="close" size={17} />
            </button>
          ) : null}
        </label>
        <div className="toolbar-stats">
          <span className="chip">{servers.length} {t("home.serversShort")}</span>
          <span className="chip">{profiles.length} {t("home.profilesShort")}</span>
          {status.running ? <span className="chip chip--success">{t("servers.connectedCount")}: 1</span> : null}
        </div>
      </Card>

      {servers.length === 0 ? (
        <Card className="empty-card">
          <EmptyState
            icon="dns"
            title={t("servers.emptyTitle")}
            text={t("servers.emptyText")}
            action={
              <Button icon="add_link" onClick={() => onNavigate("subscriptions")}>
                {t("servers.toSubscriptions")}
              </Button>
            }
          />
        </Card>
      ) : filtered.length === 0 ? (
        <Card className="empty-card">
          <EmptyState
            icon="search_off"
            title={t("servers.notFound")}
            text={t("servers.notFoundHint")}
            action={
              <Button variant="tonal" onClick={() => setQuery("")}>
                {t("common.clear")}
              </Button>
            }
          />
        </Card>
      ) : (
        <div className="server-groups">
          {groups.map(([profileName, group]) => (
            <Card key={profileName} className="server-group">
              <div className="server-group__head">
                <div>
                  <h3>{profileName}</h3>
                  <span className="muted">{group.length} {t("home.serversShort")}</span>
                </div>
              </div>
              <div className="server-list server-list--page">
                {group.map((server) => {
                  const selected = server.id === selectedServerId;
                  const connected = status.running && server.id === status.serverId;
                  return (
                    <div
                      key={server.id}
                      className={`server-row${selected ? " is-selected" : ""}${connected ? " is-connected" : ""}`}
                    >
                      <button type="button" className="server-row__main" onClick={() => selectServer(server.id)}>
                        <span className="radio" aria-hidden="true" />
                        <span className="server-row__body">
                          <span className="server-row__name">{server.name}</span>
                          <span className="server-row__meta">
                            <span className="protocol-badge">{server.protocol}</span>
                            {server.host ? `${server.host}${server.port ? `:${server.port}` : ""}` : t("servers.noHost")}
                          </span>
                        </span>
                      </button>
                      <span className="server-row__latency">
                        {server.latencyMs !== null ? `${server.latencyMs} ms` : "—"}
                      </span>
                      <Button
                        variant={connected ? "tonal" : "outlined"}
                        icon={connected ? "stop_circle" : "play_arrow"}
                        disabled={busy}
                        onClick={(event) => {
                          event.stopPropagation();
                          void (connected ? disconnect() : connect(server.id));
                        }}
                      >
                        {connected ? t("home.disconnect") : t("home.connect")}
                      </Button>
                    </div>
                  );
                })}
              </div>
            </Card>
          ))}
        </div>
      )}
    </>
  );
}
