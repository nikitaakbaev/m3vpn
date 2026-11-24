import { useMemo, useState } from "react";
import { PageHeader } from "../../components/PageHeader";
import { Card } from "../../components/Card";
import { Button } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { Segmented } from "../../components/Segmented";
import { Icon } from "../../components/Icon";
import { useAppStore, useT } from "../../store/useAppStore";
import type { Server } from "../../lib/tauri";
import type { Page } from "../../components/NavRail";

interface ServersPageProps {
  onNavigate: (page: Page) => void;
}

type SortMode = "group" | "ping" | "name";

function latencyClass(latency: number | null): string {
  if (latency === null) return "is-unknown";
  if (latency < 80) return "is-good";
  if (latency < 200) return "is-ok";
  if (latency < 500) return "is-warn";
  return "is-bad";
}

export function ServersPage({ onNavigate }: ServersPageProps) {
  const t = useT();
  const servers = useAppStore((state) => state.servers);
  const profiles = useAppStore((state) => state.profiles);
  const connectedServerId = useAppStore((state) =>
    state.status.running ? state.status.serverId : null,
  );
  const selectedServerId = useAppStore((state) => state.selectedServerId);
  const selectServer = useAppStore((state) => state.selectServer);
  const connect = useAppStore((state) => state.connect);
  const disconnect = useAppStore((state) => state.disconnect);
  const refreshData = useAppStore((state) => state.refreshData);
  const probeAll = useAppStore((state) => state.probeAll);
  const probing = useAppStore((state) => state.probing);
  const busy = useAppStore((state) => state.busy);
  const settings = useAppStore((state) => state.settings);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortMode>("group");

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return servers;
    return servers.filter((server) =>
      `${server.name} ${server.host ?? ""} ${server.profileName} ${server.protocol}`
        .toLowerCase()
        .includes(needle),
    );
  }, [servers, query]);

  const ordered = useMemo(() => {
    const list = [...filtered];
    if (sort === "ping") {
      list.sort(
        (left, right) =>
          (left.latencyMs ?? Number.MAX_SAFE_INTEGER) -
          (right.latencyMs ?? Number.MAX_SAFE_INTEGER),
      );
    } else if (sort === "name") {
      list.sort((left, right) => left.name.localeCompare(right.name, "ru"));
    }
    return list;
  }, [filtered, sort]);

  const groups = useMemo(() => {
    const map = new Map<string, Server[]>();
    for (const server of ordered) {
      const list = map.get(server.profileName) ?? [];
      list.push(server);
      map.set(server.profileName, list);
    }
    return [...map.entries()];
  }, [ordered]);

  const pingMode = settings["ping.protocol"] ?? "incy";
  const pingDisplay = settings["ping.display"] ?? "both";
  const pingLabelKey =
    pingMode === "tcp"
      ? "servers.tcpPing"
      : pingMode === "proxy_get"
        ? "servers.pingGet"
        : pingMode === "proxy_head"
          ? "servers.pingHead"
          : "servers.pingIncy";

  const renderLatency = (server: Server) => {
    const ms = server.latencyMs;
    const cls = latencyClass(ms);
    const label = ms === null ? "—" : `${ms} ms`;
    const percent =
      ms === null ? 0 : Math.max(8, Math.min(100, 100 - Math.min(ms, 600) / 6));
    if (pingDisplay === "dots") {
      return <span className={`latency-dot ${cls}`} title={label} aria-label={label} />;
    }
    if (pingDisplay === "scale") {
      return (
        <span className={`latency-bar ${cls}`} title={label} aria-label={label}>
          <span style={{ width: `${percent}%` }} />
        </span>
      );
    }
    if (pingDisplay === "both") {
      return (
        <span className="latency-cell">
          <span className={`latency-bar ${cls}`} aria-hidden="true">
            <span style={{ width: `${percent}%` }} />
          </span>
          <span className={`latency ${cls}`}>{label}</span>
        </span>
      );
    }
    return <span className={`latency ${cls}`}>{label}</span>;
  };

  const renderRow = (server: Server) => {
    const selected = server.id === selectedServerId;
    const connected = connectedServerId === server.id;
    return (
      <div
        key={server.id}
        className={`server-row${selected ? " is-selected" : ""}${connected ? " is-connected" : ""}`}
      >
        <button
          type="button"
          className="server-row__main"
          aria-pressed={selected}
          onClick={() => selectServer(server.id)}
        >
          <span className="radio" aria-hidden="true" />
          <span className="server-row__body">
            <span className="server-row__name">{server.name}</span>
            <span className="server-row__meta">
              <span className="protocol-badge">{server.protocol}</span>
              <span className="engine-badge">{server.engine}</span>
              {server.host ? `${server.host}${server.port ? `:${server.port}` : ""}` : t("servers.noHost")}
            </span>
          </span>
        </button>
        {renderLatency(server)}
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
  };

  const renderList = (items: Server[]) => (
    <div className="server-list server-list--page">{items.map(renderRow)}</div>
  );

  return (
    <>
      <PageHeader
        title={t("servers.title")}
        subtitle={t("servers.subtitle")}
        badge={<span className="pill pill--accent">M9</span>}
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

      <Card className="toolbar-card toolbar-card--stack">
        <div className="toolbar-row">
          <label className="search">
            <Icon name="search" size={19} />
            <input
              aria-label={t("servers.search")}
              placeholder={t("servers.search")}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            {query ? (
              <button
                type="button"
                className="search__clear"
                onClick={() => setQuery("")}
                aria-label={t("common.clear")}
              >
                <Icon name="close" size={17} />
              </button>
            ) : null}
          </label>
          <div className="toolbar-stats">
            <span className="chip">
              {servers.length} {t("home.serversShort")}
            </span>
            <span className="chip">
              {profiles.length} {t("home.profilesShort")}
            </span>
            {connectedServerId !== null ? (
              <span className="chip chip--success">
                {t("servers.connectedCount")}: 1
              </span>
            ) : null}
          </div>
        </div>

        <div className="toolbar-row toolbar-row--between">
          <Segmented
            value={sort}
            label={t("servers.sortPing")}
            options={[
              { value: "group", label: t("servers.sortGroup"), icon: "workspaces" },
              { value: "ping", label: t("servers.sortPing"), icon: "speed" },
              { value: "name", label: t("servers.sortName"), icon: "sort_by_alpha" },
            ]}
            onChange={(value) => setSort(value as SortMode)}
          />
          <div className="toolbar-actions">
            <span className="muted toolbar-note">{t(pingLabelKey)}</span>
            <Button
              variant="tonal"
              icon={probing ? "progress_activity" : "speed"}
              disabled={probing || servers.length === 0}
              onClick={() => void probeAll()}
            >
              {probing ? t("servers.probing") : t("servers.probe")}
            </Button>
            <Button
              icon="bolt"
              disabled={probing || busy || servers.length === 0}
              onClick={() => void probeAll(true)}
            >
              {t("servers.fastest")}
            </Button>
          </div>
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
      ) : sort === "group" ? (
        <div className="server-groups">
          {groups.map(([profileName, group]) => (
            <Card key={profileName} className="server-group">
              <div className="server-group__head">
                <div>
                  <h3>{profileName}</h3>
                  <span className="muted">
                    {group.length} {t("home.serversShort")}
                  </span>
                </div>
              </div>
              {renderList(group)}
            </Card>
          ))}
        </div>
      ) : (
        <Card className="server-group">
          <div className="server-group__head">
            <div>
              <h3>
                {sort === "ping" ? t("servers.sortPing") : t("servers.sortName")}
              </h3>
              <span className="muted">{ordered.length}</span>
            </div>
          </div>
          {renderList(ordered)}
        </Card>
      )}
    </>
  );
}
