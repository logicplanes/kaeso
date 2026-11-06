import { useMemo } from "react";
import { useInfiniteQuery, useQuery, type UseQueryResult } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  DataTable,
  Empty,
  FilterChip,
  JsonView,
  KeyValue,
  Section,
  Segmented,
  Select,
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
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyName } from "../keys/party";
import { ranges, type LogsSearch } from "./logs-page";
import { toolCallStatus as statusOf, toolErrorLabel as errorLabel, TOOL_CALL_STATUSES } from "~/format/labels";

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

export function ToolCallsPage({ org, project, search }: { org: string; project: string; search: LogsSearch }) {
  const navigate = useNavigate();
  const partyName = usePartyName(org, project);
  const filters: ToolCallFilters = {
    ...(search.range ? { range: search.range } : {}),
    ...(search.from && search.to ? { from: search.from, to: search.to } : {}),
    ...(search.call_status ? { status: search.call_status } : {}),
    ...(search.call_tool ? { tool: search.call_tool } : {}),
  };
  const calls = useInfiniteQuery(toolCallsInfiniteQuery(org, project, filters));
  const rows = useMemo(() => calls.data?.pages.flatMap((p) => p.data) ?? [], [calls.data]);
  const setSearch = (patch: SearchPatch<LogsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: patchSearch(search, patch), replace: true });
  };
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
  return (
    <>
      <ProjectHeader org={org} project={project} page={t("logs.title", "Request logs")} path="logs" />
      <Toolbar aria-label={t("logs.tools.filtersLabel", "Filter tool calls")}>
        <KindSwitch value="tools" onChange={(kind) => setSearch({ kind, call: undefined, call_status: undefined, call_tool: undefined })} />
        <div className="shrink-0">
          <Select
            aria-label={t("logs.timeRange", "Time range")}
            size="sm"
            icon={icons.calendar}
            value={search.range ?? "24h"}
            onValueChange={(v) => setSearch({ range: ranges().find((r) => r.value === v && v !== "24h")?.value, from: undefined, to: undefined })}
            options={ranges()}
          />
        </div>
        {search.call_tool && (
          <FilterChip
            property={t("logs.tools.tool", "Tool")}
            operator={t("logs.filter.is", "is")}
            value={search.call_tool}
            onRemove={() => setSearch({ call_tool: undefined })}
          />
        )}
        <div className="shrink-0">
          <Select
            aria-label={common.status()}
            size="sm"
            value={search.call_status ?? "all"}
            onValueChange={(v) => setSearch({ call_status: v === "ok" || v === "error" || v === "refused" ? v : undefined })}
            options={[
              { value: "all", label: t("logs.tools.status.all", "Every status") },
              ...TOOL_CALL_STATUSES.map((value) => ({ value, label: statusOf(value).label })),
            ]}
          />
        </div>
      </Toolbar>
      <AppFrame.Body padding="none">
        <DataTable
          aria-label={t("logs.tools.label", "Tool calls")}
          data={rows}
          columns={columns}
          getRowId={(r) => r.id}
          loading={calls.isPending}
          error={calls.isError && !unavailable && rows.length === 0 ? t("logs.tools.loadFailed", "Tool calls couldn't load") : undefined}
          onRetry={() => void calls.refetch()}
          empty={
            <Empty
              title={unavailable ? t("logs.unavailable", "Request logs are unavailable for now") : t("logs.tools.empty", "No tool calls in this range")}
              description={
                unavailable
                  ? t("logs.unavailableHint", "Usage and limits still work.")
                  : t("logs.tools.emptyHint", "Calls appear here as clients use the project's MCP portal.")
              }
            />
          }
          hasMore={calls.hasNextPage}
          loadingMore={calls.isFetchingNextPage}
          onLoadMore={() => void calls.fetchNextPage()}
          onRowClick={(r) => setSearch({ call: r.id })}
          activeRowId={search.call}
        />
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
