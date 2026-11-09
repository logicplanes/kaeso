import { useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery, type UseQueryResult } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Button,
  DataTable,
  DisplayOptions,
  Empty,
  JsonView,
  KeyValue,
  Section,
  Segmented,
  Sheet,
  Skeleton,
  Status,
  Toolbar,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatBytes, formatDuration } from "@logicplanes/kaeso-ui/format";
import { isApiError } from "~/api/errors";
import type { ToolCallEntryView, ToolCallView } from "~/api/generated/types";
import { toolCallQuery, toolCallsInfiniteQuery, type ToolCallFilters } from "~/api/queries/logs";
import { projectServersQuery, projectToolsQuery } from "~/api/queries/tools";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePhone } from "~/shell/use-phone";
import { usePartyName } from "../keys/party";
import type { LogsSearch } from "./logs-page";
import { ToolCallsExportDialog } from "./export-dialog";
import { useLiveToolCalls } from "./live";
import {
  FilterBar,
  IdSearch,
  LiveButton,
  RangeControls,
  useLoadMoreNearEnd,
  usePartyFilters,
  useRowStepping,
  densityRow,
  type Density,
  type FilterDef,
} from "./toolbar";
import { toolCallStatus as statusOf, toolErrorLabel as errorLabel, TOOL_CALL_STATUSES, TOOL_ERROR_TYPES } from "~/format/labels";

export function KindSwitch({ value, onChange }: { value: "models" | "tools"; onChange: (kind: "tools" | undefined) => void }) {
  return (
    <div className="shrink-0">
      <Segmented
        size="sm"
        aria-label={t("logs.kind", "What to show")}
        value={value}
        onValueChange={(v) => onChange(v === "tools" ? "tools" : undefined)}
        options={[
          { value: "models", label: t("logs.kind.models", "Models") },
          { value: "tools", label: t("logs.kind.tools", "Tools") },
        ]}
      />
    </div>
  );
}

type ToolFilterId = "key" | "member" | "server" | "tool" | "status" | "error";
const TOOL_FILTER_IDS: ToolFilterId[] = ["key", "member", "server", "tool", "status", "error"];
const SEARCH_OF: Record<ToolFilterId, keyof LogsSearch> = {
  key: "key",
  member: "member",
  server: "call_server",
  tool: "call_tool",
  status: "call_status",
  error: "call_error",
};

export function ToolCallsPage({ org, project, search }: { org: string; project: string; search: LogsSearch }) {
  const navigate = useNavigate();
  const partyName = usePartyName(org, project);
  const phone = usePhone();
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const [exporting, setExporting] = useState(false);
  const { keyName, keyOptions, memberOptions } = usePartyFilters(org, project);
  const tools = useQuery(projectToolsQuery(org, project));
  const servers = useQuery(projectServersQuery(org, project));
  const [density, setDensity] = useState<Density>("default");
  const scroller = useRef<HTMLDivElement | null>(null);
  const filters: ToolCallFilters = {
    ...(search.range ? { range: search.range } : {}),
    ...(search.from && search.to ? { from: search.from, to: search.to } : {}),
    ...(search.key ? { key: search.key } : {}),
    ...(search.member ? { member: search.member } : {}),
    ...(search.call_server ? { server: search.call_server } : {}),
    ...(search.call_tool ? { tool: search.call_tool } : {}),
    ...(search.call_status ? { status: search.call_status } : {}),
    ...(search.call_error ? { error: search.call_error } : {}),
  };
  const calls = useInfiniteQuery(toolCallsInfiniteQuery(org, project, filters));
  const base = useMemo(() => calls.data?.pages.flatMap((p) => p.data) ?? [], [calls.data]);
  const { rows, pending, fresh, cut, showPending } = useLiveToolCalls({ org, project, filters, live: !!search.live, base, scroller });
  const hasNextPage = calls.hasNextPage && !cut;
  const setSearch = (patch: SearchPatch<LogsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: patchSearch(search, patch), replace: true });
  };
  useLoadMoreNearEnd(scroller, { ...calls, hasNextPage });
  useRowStepping(rows, search.call, (id) => setSearch({ call: id }));

  const serverName = (id: string) => servers.data?.find((s) => s.id === id)?.name ?? id;
  const filterDefs: FilterDef[] = TOOL_FILTER_IDS.map((id): FilterDef => {
    const value = search[SEARCH_OF[id]] as string | undefined;
    switch (id) {
      case "key":
        return { id, label: t("logs.filter.key", "Key"), value, values: keyOptions, valueLabel: keyName, searchable: true };
      case "member":
        return { id, label: t("logs.filter.member", "Member"), value, values: memberOptions, searchable: true };
      case "server":
        return {
          id,
          label: t("logs.tools.server", "MCP server"),
          value,
          values: (servers.data ?? []).map((s) => ({ value: s.id, label: s.name })),
          valueLabel: serverName,
          searchable: true,
        };
      case "tool":
        return {
          id,
          label: t("logs.tools.tool", "Tool"),
          value,
          values: (tools.data ?? [])
            .filter((tool) => !search.call_server || tool.server === search.call_server)
            .map((tool) => ({ value: tool.exposed_name, label: tool.exposed_name })),
          searchable: true,
          width: "lg",
        };
      case "status":
        return { id, label: common.status(), value, values: TOOL_CALL_STATUSES.map((s) => ({ value: s, label: statusOf(s).label })) };
      case "error":
        return { id, label: t("logs.filter.error", "Error kind"), value, values: TOOL_ERROR_TYPES.map((e) => ({ value: e, label: errorLabel(e) })) };
    }
  });
  const setFilter = (id: string, v: string | undefined) => {
    if (id === "status") setSearch({ call_status: v === "ok" || v === "error" || v === "refused" ? v : undefined, call: undefined });
    else setSearch({ [SEARCH_OF[id as ToolFilterId]]: v, call: undefined });
  };
  const filtered = filterDefs.some((f) => f.value !== undefined);

  const columns = useMemo(
    () => [
      col.date({ id: "time", header: t("logs.column.started", "Started"), size: 176, priority: 1 }, (r: ToolCallView) => r.started_at),
      col.mono({ id: "tool", header: t("logs.tools.tool", "Tool"), size: 240, priority: 1 }, (r: ToolCallView) => r.tool),
      col.status({ id: "status", header: common.status(), size: 120, priority: 1 }, (r: ToolCallView) => statusOf(r.status)),
      col.text({ id: "caller", header: t("logs.tools.caller", "Caller"), size: 180, priority: 2 }, (r: ToolCallView) => partyName(r.caller)),
      col.text({ id: "client", header: t("logs.tools.client", "Client"), size: 160, priority: 3 }, (r: ToolCallView) => r.client || "—"),
      col.duration({ id: "duration", header: t("logs.column.duration", "Duration"), size: 112, priority: 2 }, (r: ToolCallView) => r.duration_ms),
      col.custom(
        { id: "server", header: t("logs.tools.serverTime", "Server time"), size: 120, priority: 3 },
        (r: ToolCallView) => r.upstream_ms ?? null,
        (v) => (v === null ? "—" : formatDuration(v)),
        { align: "end" },
      ),
    ],
    [partyName],
  );
  const unavailable = calls.error && isApiError(calls.error) && calls.error.type === "unavailable";
  const canExport = rights.has("logs.export");
  const nothingToExport = calls.isSuccess && rows.length === 0;
  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("logs.title", "Request logs")}
        path="logs"
        actions={
          <Button
            size="sm"
            leadingIcon={icons.download}
            disabled={!canExport || nothingToExport}
            disabledReason={!canExport ? needs("logs.export") : nothingToExport ? t("logs.tools.noMatch", "No tool calls match these filters") : undefined}
            onClick={() => showExportProgress() || setExporting(true)}
          >
            {common.export()}
          </Button>
        }
      />
      {canExport && (
        <ToolCallsExportDialog
          org={org}
          project={project}
          filters={filters}
          names={{
            key: filters.key && keyName(filters.key),
            member: memberOptions.find((m) => m.value === filters.member)?.label,
            server: filters.server && serverName(filters.server),
          }}
          labels={{
            ...(filters.status ? { status: statusOf(filters.status).label } : {}),
            ...(filters.error ? { error: errorLabel(filters.error) } : {}),
          }}
          canContent={rights.has("logs.content.view")}
          open={exporting}
          onOpenChange={setExporting}
        />
      )}
      <Toolbar aria-label={t("logs.tools.filtersLabel", "Filter tool calls")}>
        <KindSwitch value="tools" onChange={(kind) => setSearch({ kind, call: undefined })} />
        <RangeControls value={search} onChange={setSearch} />
        <FilterBar filters={filterDefs} onChange={setFilter} compact={phone} />
        <Toolbar.Spacer />
        <IdSearch
          label={t("logs.tools.openById", "Open a tool call by id")}
          placeholder={t("logs.tools.callId", "Call id")}
          onOpen={(id) => setSearch({ call: id })}
        />
        <LiveButton live={!!search.live} onChange={(on) => setSearch({ live: on || undefined })} />
        <DisplayOptions rows={[densityRow(density, setDensity)]} />
      </Toolbar>
      <AppFrame.Body padding="none">
        <div ref={scroller} className="flex min-h-0 flex-col">
          {pending.length > 0 && (
            <div className="my-8 flex justify-center">
              <Button size="sm" onClick={showPending}>
                {t("logs.tools.showNew", "{count, plural, one {Show # new tool call} other {Show # new tool calls}}", { count: pending.length })}
              </Button>
            </div>
          )}
          <DataTable
            aria-label={t("logs.tools.label", "Tool calls")}
            data={rows}
            columns={columns}
            getRowId={(r) => r.id}
            density={density}
            loading={calls.isPending}
            error={calls.isError && !unavailable && rows.length === 0 ? t("logs.tools.loadFailed", "Tool calls couldn't load") : undefined}
            onRetry={() => void calls.refetch()}
            empty={
              <Empty
                title={
                  unavailable
                    ? t("logs.unavailable", "Request logs are unavailable for now")
                    : filtered
                      ? t("logs.tools.noMatch", "No tool calls match these filters")
                      : t("logs.tools.empty", "No tool calls in this range")
                }
                description={
                  unavailable
                    ? t("logs.unavailableHint", "Usage and limits still work.")
                    : filtered
                      ? t("logs.noMatchHint", "Remove a filter or widen the time range.")
                      : t("logs.tools.emptyHint", "Calls appear here as clients use the project's MCP portal.")
                }
              />
            }
            hasMore={hasNextPage}
            loadingMore={calls.isFetchingNextPage}
            onLoadMore={() => void calls.fetchNextPage()}
            onRowClick={(r) => setSearch({ call: r.id })}
            activeRowId={search.call}
            freshRowIds={fresh}
          />
        </div>
      </AppFrame.Body>
      <ToolCallSheet org={org} project={project} callId={search.call} onClose={() => setSearch({ call: undefined })} />
    </>
  );
}

function ToolCallSheet({ org, project, callId, onClose }: { org: string; project: string; callId: string | undefined; onClose: () => void }) {
  const call = useQuery({ ...toolCallQuery(org, project, callId ?? ""), enabled: !!callId });
  return (
    <Sheet
      open={!!callId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="lg"
      title={call.data?.tool ?? t("logs.tools.call", "Tool call")}
      subtitle={callId}
    >
      {callId && <CallBody key={callId} org={org} project={project} call={call} />}
    </Sheet>
  );
}

function CallBody({ org, project, call }: { org: string; project: string; call: UseQueryResult<ToolCallEntryView> }) {
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const partyName = usePartyName(org, project);
  const c = call.data;
  if (call.isError) return <Empty title={t("logs.tools.callFailed", "This tool call couldn't load")} description={call.error.message} />;
  if (!c) return <Skeleton lines={4} />;
  const status = statusOf(c.status);
  return (
    <div className="flex flex-col gap-24">
      <Section title={t("logs.inspector.outcome", "Outcome")} spacing="none">
        <KeyValue
          layout="stacked"
          className="grid grid-cols-2 gap-x-32"
          items={[
            { label: common.status(), value: <Status tone={status.tone}>{status.label}</Status> },
            ...(c.error_type ? [{ label: t("logs.inspector.error", "Error"), value: errorLabel(c.error_type) }] : []),
            ...(c.error_message ? [{ label: t("logs.inspector.message", "Message"), value: c.error_message, wrap: true }] : []),
            { label: t("logs.column.duration", "Duration"), value: formatDuration(c.duration_ms) },
            { label: t("logs.tools.serverTime", "Server time"), value: c.upstream_ms == null ? "—" : formatDuration(c.upstream_ms) },
          ]}
        />
      </Section>
      <Section title={t("logs.inspector.sentBy", "Sent by")} spacing="none">
        <KeyValue
          layout="stacked"
          className="grid grid-cols-2 gap-x-32"
          items={[
            { label: t("logs.tools.caller", "Caller"), value: partyName(c.caller) },
            { label: t("logs.tools.client", "Client"), value: c.client || "—" },
            { label: t("logs.tools.onServer", "Name on the server"), value: c.upstream_tool, mono: true },
            { label: t("logs.tools.protocol", "MCP version"), value: c.protocol_version, mono: true },
            { label: t("logs.tools.sizes", "Request and answer"), value: `${formatBytes(c.request_bytes)} · ${formatBytes(c.response_bytes)}` },
          ]}
        />
      </Section>
      {!rights.has("logs.content.view") ? (
        <Empty title={t("logs.inspector.noContentRight", "You can't see request content")} description={needs("logs.content.view")} />
      ) : c.content ? (
        <>
          <Section title={t("logs.tools.arguments", "Arguments")} spacing="none">
            <JsonView value={c.content.arguments} defaultDepth={4} />
          </Section>
          <Section title={t("logs.tools.result", "Answer")} spacing="none">
            <JsonView value={c.content.result} defaultDepth={4} />
          </Section>
        </>
      ) : (
        <Empty
          title={t("logs.inspector.notLogged", "Content not logged")}
          description={t("logs.inspector.notLoggedHint", "Logging was off when this request ran, or its content has passed the retention period.")}
        />
      )}
    </div>
  );
}
