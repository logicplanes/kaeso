import type { QueryClient } from "@tanstack/react-query";
import { granularity } from "~/format/buckets";
import type { PeriodSearch } from "~/format/period";
import { resolvePeriod } from "~/format/presets";
import { auditLogQuery, type AuditFilters } from "./audit";
import { logsInfiniteQuery, type LogFilters } from "./logs";
import { projectUsageQuery, usageSpanQuery } from "./usage";

export function prefetchLogs(client: QueryClient, org: string, project: string, filters: LogFilters): Promise<void> {
  return client.prefetchInfiniteQuery(logsInfiniteQuery(org, project, filters));
}

export function prefetchProjectAuditLog(client: QueryClient, org: string, project: string, filters: AuditFilters): Promise<void> {
  return client.prefetchInfiniteQuery(auditLogQuery({ org, project }, filters));
}

export async function prefetchProjectOverview(client: QueryClient, org: string, project: string, search: PeriodSearch): Promise<void> {
  const span = search.period === "all" ? await client.ensureQueryData(usageSpanQuery(org, project)).catch(() => undefined) : undefined;
  if (search.period === "all" && !span) return;
  const { from, to } = resolvePeriod(search, span?.first_day);
  await client.prefetchQuery(projectUsageQuery(org, project, from, to, granularity(from, to)));
}
