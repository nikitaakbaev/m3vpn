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
import { ServerLatency, type PingDisplay } from "./ServerLatency";

interface ServersPageProps {
  onNavigate: (page: Page) => void;
}

type SortMode = "group" | "ping" | "name";
type ServerFilter = "all" | "active" | "excluded";

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
  const setServerExcluded = useAppStore((state) => state.setServerExcluded);
  const setServersExcluded = useAppStore((state) => state.setServersExcluded);
  const probing = useAppStore((state) => state.probing);
  const busy = useAppStore((state) => state.busy);
  const settings = useAppStore((state) => state.settings);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ServerFilter>("all");
  const [bulk, setBulk] = useState(false);
  const [bulkSelection, setBulkSelection] = useState<Set<string>>(new Set());
  const configuredSort = settings["servers.sort"];
  const sortMode: SortMode =
    configuredSort === "ping" || configuredSort === "name" ? configuredSort : "group";

  const filtered = useMemo(() => {
    const byMode = servers.filter((server) => {
      if (filter === "excluded") return server.excluded;
      if (filter === "active") return !server.excluded;
      return true;
    });
    const needle = query.trim().toLowerCase();
    if (!needle) return byMode;
    return byMode.filter((server) =>
      `${server.name} ${server.host ?? ""} ${server.profileName} ${server.protocol} ${server.engine}`
        .toLowerCase()
        .includes(needle),
    );
  }, [servers, query, filter]);

  const ordered = useMemo(() => {
    const list = [...filtered];
    list.sort((left, right) => {
      if (left.excluded !== right.excluded) {
        return left.excluded ? 1 : -1;
      }
      if (sortMode === "ping") {
        return (
          (left.latencyMs ?? Number.MAX_SAFE_INTEGER) -
          (right.latencyMs ?? Number.MAX_SAFE_INTEGER)
        );
      }
      if (sortMode === "name") {
        return left.name.localeCompare(right.name, "ru");
      }
      return 0;
    });
    return list;
  }, [filtered, sortMode]);

  const groups = useMemo(() => {
    const map = new Map<string, Server[]>();
    for (const server of ordered) {
      const list = map.get(server.profileName) ?? [];
      list.push(server);
      map.set(server.profileName, list);
    }
    return [...map.entries()];
  }, [ordered]);

  const eligibleCount = servers.filter((server) => !server.excluded).length;
  const excludedCount = servers.length - eligibleCount;
  const sortLabelKey =
    sortMode === "ping"
      ? "servers.sortPing"
      : sortMode === "name"
        ? "servers.sortName"
        : "servers.sortGroup";
  const pingMode = settings["ping.protocol"] ?? "fast_http";
  const pingDisplay = (settings["ping.display"] ?? "both") as PingDisplay;
  const pingLabelKey =
    pingMode === "tcp"
      ? "servers.tcpPing"
      : pingMode === "proxy_get"
        ? "servers.pingGet"
        : pingMode === "proxy_head"
          ? "servers.pingHead"
          : "servers.pingFastHttp";

  const toggleSelected = (serverId: string) => {
    setBulkSelection((current) => {
      const next = new Set(current);
      if (next.has(serverId)) next.delete(serverId);
      else next.add(serverId);
      return next;
    });
  };

  const toggleBulk = () => {
    setBulk((current) => !current);
    setBulkSelection(new Set());
  };

  const changeFilter = (value: ServerFilter) => {
    setFilter(value);
    setBulkSelection(new Set());
  };

  const applyBulk = async (excluded: boolean) => {
    if (bulkSelection.size === 0) return;
    if (await setServersExcluded([...bulkSelection], excluded)) {
      setBulkSelection(new Set());
    }
  };

  const renderRow = (server: Server) => {
    const selected = server.id === selectedServerId;
    const connected = connectedServerId === server.id;
    const checked = bulkSelection.has(server.id);
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
        {server.excluded ? (
          <span className="pill pill--muted">{t("servers.excluded")}</span>
        ) : (
          <ServerLatency latency={server.latencyMs} display={pingDisplay} />
        )}
        <button
          type="button"
          className="icon-button"
          title={server.excluded ? t("servers.include") : t("servers.exclude")}
          aria-label={server.excluded ? t("servers.include") : t("servers.exclude")}
          onClick={(event) => {
            event.stopPropagation();
            void setServerExcluded(server.id, !server.excluded);
          }}
        >
          <Icon name={server.excluded ? "visibility" : "visibility_off"} size={18} />
        </button>
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
            {excludedCount > 0 ? (
              <span className="chip">{t("servers.excludedCount")}: {excludedCount}</span>
            ) : null}
            {connectedServerId !== null ? (
              <span className="chip chip--success">
                {t("servers.connectedCount")}: 1
              </span>
            ) : null}
          </div>
        </div>

        <div className="toolbar-row toolbar-row--between">
          <div className="toolbar-left">
            <Button variant="text" icon="tune" onClick={() => onNavigate("settings")}>
              {t("servers.sortedBy")}: {t(sortLabelKey)}
            </Button>
            <Segmented
              value={filter}
              label={t("servers.filter")}
              options={[
                { value: "all", label: t("servers.filterAll") },
                { value: "active", label: t("servers.filterActive") },
                { value: "excluded", label: t("servers.filterExcluded") },
              ]}
              onChange={(value) => changeFilter(value as ServerFilter)}
            />
          </div>
          <div className="toolbar-actions">
            <span className="muted toolbar-note">{t(pingLabelKey)}</span>
            <Button
              variant="tonal"
              icon={probing ? "progress_activity" : "speed"}
              disabled={probing || bulk || eligibleCount === 0}
              onClick={() => void probeAll()}
            >
              {probing ? t("servers.probing") : t("servers.probe")}
            </Button>
            <Button
              icon="bolt"
              disabled={probing || busy || bulk || eligibleCount === 0}
              onClick={() => void probeAll(true)}
            >
              {t("servers.fastest")}
            </Button>
            <Button
              variant={bulk ? "tonal" : "text"}
              icon={bulk ? "done" : "checklist"}
              onClick={toggleBulk}
            >
              {bulk ? t("servers.bulkDone") : t("servers.bulk")}
            </Button>
          </div>
        </div>
        {bulk ? (
          <div className="toolbar-row toolbar-bulk">
            <span className="muted">
              {t("servers.selected")}: <strong>{bulkSelection.size}</strong>
            </span>
            <div className="toolbar-actions">
              <Button
                variant="text"
                onClick={() => setBulkSelection(new Set(filtered.map((server) => server.id)))}
              >
                {t("servers.selectAll")}
              </Button>
              <Button variant="text" onClick={() => setBulkSelection(new Set())}>
                {t("servers.clearSelection")}
              </Button>
              <Button
                variant="outlined"
                icon="visibility_off"
                disabled={bulkSelection.size === 0}
                onClick={() => void applyBulk(true)}
              >
                {t("servers.excludeSelected")}
              </Button>
              <Button
                variant="outlined"
                icon="visibility"
                disabled={bulkSelection.size === 0}
                onClick={() => void applyBulk(false)}
              >
                {t("servers.includeSelected")}
              </Button>
            </div>
          </div>
        ) : null}
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
            title={t(
              query.trim()
                ? "servers.notFound"
                : filter === "excluded"
                  ? "servers.noExcluded"
                  : filter === "active"
                    ? "servers.noActive"
                    : "servers.notFound",
            )}
            text={t("servers.notFoundHint")}
            action={
              <Button
                variant="tonal"
                onClick={() => {
                  setQuery("");
                  changeFilter("all");
                }}
              >
                {t("common.clear")}
              </Button>
            }
          />
        </Card>
      ) : sortMode === "group" ? (
        <div className="server-groups">
          {groups.map(([profileName, group]) => (
            <Card key={profileName} className="server-group">
              <div className="server-group__head">
                <div>
                  <h2>{profileName}</h2>
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
              <h2>
                {sortMode === "ping" ? t("servers.sortPing") : t("servers.sortName")}
              </h2>
              <span className="muted">{ordered.length}</span>
            </div>
          </div>
          {renderList(ordered)}
        </Card>
      )}
    </>
  );
}
