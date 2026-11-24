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

// The URL may read an amount as a number; at most the log's 12 decimal places count, and the
// gateway refuses anything else.
const minCost = (v: unknown): string | undefined => {
  const text = typeof v === "number" ? String(v) : v;
  return typeof text === "string" && /^\d+(\.\d{1,12})?$/.test(text) ? text : undefined;
};

function validateLogsSearch(s: Record<string, unknown>): LogsSearch {
  const from = parseDay(s.from);
  const to = parseDay(s.to);
  const status = Number(s.status);
  const stream = bool(s.stream);
  const failed = bool(s.failed);
  const slower = Number(s.min_duration_ms);
  const cost = minCost(s.min_cost);
  const out: LogsSearch = {
    ...(typeof s.range === "string" && RANGES.has(s.range) ? { range: s.range as LogRange } : {}),
    ...(from && to && from <= to ? { from, to, ...(bool(s.utc) ? { utc: true } : {}) } : {}),
    ...text("key", s.key),
    ...text("member", s.member),
    ...text("model", s.model),
    ...text("provider", s.provider),
    ...(Number.isInteger(status) && status >= 100 && status < 600 ? { status } : {}),
    ...(failed !== undefined ? { failed } : {}),
    ...text("error", s.error),
    ...(stream !== undefined ? { stream } : {}),
    ...(s.source === "api" || s.source === "playground" || s.source === "observed" ? { source: s.source } : {}),
    ...text("source_id", s.source_id),
    ...text("ingress", s.ingress),
    ...text("egress", s.egress),
    ...text("alias", s.alias),
    ...(bool(s.fell_back) ? { fell_back: true } : {}),
    ...text("endpoint", s.endpoint),
    ...text("requested_model", s.requested_model),
    ...text("tag", s.tag),
    ...text("error_message", s.error_message),
    ...text("client", s.client),
    ...(s.guardrail_action === "allow" || s.guardrail_action === "block" || s.guardrail_action === "replace" ? { guardrail_action: s.guardrail_action } : {}),
    ...(s.response_cache === "hit" || s.response_cache === "miss" || s.response_cache === "bypass" ? { response_cache: s.response_cache } : {}),
    ...(Number.isInteger(slower) && slower > 0 ? { min_duration_ms: slower } : {}),
    ...(cost ? { min_cost: cost } : {}),
    ...text("log", s.log),
    ...(s.kind === "tools" ? { kind: "tools" as const } : {}),
    ...text("call", s.call),
    ...text("call_tool", s.call_tool),
    ...text("call_server", s.call_server),
    ...text("call_error", s.call_error),
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
    if (context.queryClient.getQueryData(sessionQuery.queryKey)?.request_logs === false) return;
    void import("~/api/queries/route-prefetch").then((m) =>
      deps.kind === "tools"
        ? m.prefetchToolCalls(context.queryClient, params.org, params.project, deps)
        : m.prefetchLogs(context.queryClient, params.org, params.project, deps),
    );
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
