import { createFileRoute } from "@tanstack/react-router";
import type { LogRange } from "~/api/queries/logs";
import { sessionQuery, useRequestLogs } from "~/auth/session";
import { LogsOff } from "~/features/project/logs/logs-off";
import { LogsPage, type LogsSearch } from "~/features/project/logs/logs-page";
import { ToolCallsPage } from "~/features/project/logs/tool-calls";
import { parseDay } from "~/format/period-search";

const RANGES = new Set(["1h", "7d", "30d"]);
const text = <K extends string>(name: K, v: unknown): Partial<Record<K, string>> => (typeof v === "string" && v ? ({ [name]: v } as Record<K, string>) : {});
const bool = (v: unknown) => (v === true || v === "true" ? true : v === false || v === "false" ? false : undefined);

function validateLogsSearch(s: Record<string, unknown>): LogsSearch {
  const from = parseDay(s.from);
  const to = parseDay(s.to);
  const status = Number(s.status);
  const stream = bool(s.stream);
  const out: LogsSearch = {
    ...(typeof s.range === "string" && RANGES.has(s.range) ? { range: s.range as LogRange } : {}),
    ...(from && to && from <= to ? { from, to } : {}),
    ...text("key", s.key),
    ...text("member", s.member),
    ...text("model", s.model),
    ...text("provider", s.provider),
    ...(Number.isInteger(status) && status >= 100 && status < 600 ? { status } : {}),
    ...(bool(s.failed) ? { failed: true } : {}),
    ...text("error", s.error),
    ...(stream !== undefined ? { stream } : {}),
    ...(s.source === "api" || s.source === "playground" ? { source: s.source } : {}),
    ...text("ingress", s.ingress),
    ...text("egress", s.egress),
    ...text("log", s.log),
    ...(s.kind === "tools" ? { kind: "tools" as const } : {}),
    ...text("call", s.call),
    ...text("call_tool", s.call_tool),
    ...(s.call_status === "ok" || s.call_status === "error" || s.call_status === "refused" ? { call_status: s.call_status } : {}),
    ...(bool(s.live) ? { live: true } : {}),
  };
  return out;
}

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/logs")({
  validateSearch: validateLogsSearch,
  loaderDeps: ({ search }) => {
    const { log: _log, live: _live, call: _call, ...filters } = validateLogsSearch(search);
    return filters;
  },
  loader: ({ context, params, deps }) => {
    if (context.queryClient.getQueryData(sessionQuery.queryKey)?.request_logs === false || deps.kind === "tools") return;
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchLogs(context.queryClient, params.org, params.project, deps));
  },
  component: Logs,
});

function Logs() {
  const { org, project } = Route.useParams();
  const search = Route.useSearch({ select: validateLogsSearch, structuralSharing: true });
  if (!useRequestLogs()) return <LogsOff org={org} project={project} />;
  if (search.kind === "tools") return <ToolCallsPage org={org} project={project} search={search} />;
  return <LogsPage org={org} project={project} search={search} />;
}
