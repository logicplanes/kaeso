import type { QueryClient } from "@tanstack/react-query";
import { loadedProjectRights } from "~/auth/rights";
import { loadedOrgRights } from "~/auth/session";
import { analyticsGranularity } from "~/format/buckets";
import { previousPeriod, type PeriodSearch } from "~/format/period";
import { comparedWith, resolvePeriod } from "~/format/presets";
import { auditLogQuery, type AuditFilters, type AuditScope } from "./audit";
import { logsInfiniteQuery, toolCallFilters, toolCallsInfiniteQuery, type LogFilters, type ToolCallSearch } from "./logs";
import { sourcesQuery } from "./sources";
import { projectServersQuery, projectToolsQuery } from "./tools";
import { orgUsageQuery, projectUsageQuery, usageSpanQuery, type GroupBy } from "./usage";

export function prefetchLogs(client: QueryClient, org: string, project: string, filters: LogFilters): Promise<void> {
  return client.prefetchInfiniteQuery(logsInfiniteQuery(org, project, filters));
}

/** The tool calls with the address's filters, and the tools, servers and sources their rows and filters name. */
export async function prefetchToolCalls(client: QueryClient, org: string, project: string, search: ToolCallSearch): Promise<void> {
  const rights = loadedProjectRights(client, org, project);
  await Promise.all([
    client.prefetchInfiniteQuery(toolCallsInfiniteQuery(org, project, toolCallFilters(search))),
    client.prefetchQuery(projectToolsQuery(org, project)),
    client.prefetchQuery(projectServersQuery(org, project)),
    rights.has("sources.view") && client.prefetchQuery(sourcesQuery(org, project)),
  ]);
}

export function prefetchAuditLog(client: QueryClient, scope: AuditScope, filters: AuditFilters): Promise<void> {
  return client.prefetchInfiniteQuery(auditLogQuery(scope, filters));
}

/**
 * The usage an analytics page waits for before it draws its chart: the period by day or week, and the period before it
 * when the page compares. The same queries `useOverview` asks for, so the page finds them in the cache.
 */
export async function prefetchAnalytics(client: QueryClient, scope: { org: string; project?: string | undefined }, search: PeriodSearch): Promise<void> {
  const { org, project } = scope;
  if (!project && !loadedOrgRights(client, org).has("usage.org.view")) return;
  const span = search.period === "all" ? await client.ensureQueryData(usageSpanQuery(org, project)).catch(() => undefined) : undefined;
  if (search.period === "all" && !span) return;
  const { from, to, period } = resolvePeriod(search, span?.first_day);
  const before = comparedWith(period, from, to) ? previousPeriod(from, to) : undefined;
  const usage = (f: string, t: string, by: GroupBy) => (project ? projectUsageQuery(org, project, f, t, by) : orgUsageQuery(org, f, t, by));
  const g = analyticsGranularity(from, to);
  await Promise.all([usage(from, to, g), ...(before ? [usage(before.from, before.to, g)] : [])].map((q) => client.prefetchQuery(q)));
}
