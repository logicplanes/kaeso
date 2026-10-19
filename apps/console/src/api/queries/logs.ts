import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { endOfDay } from "~/format/period";
import { api, data } from "../client";
import type { LogEntryView, LogView } from "../generated/types";
import { nextPageParam, refetchOnFocusWhileOnePage } from "../paging";

export type LogRange = "1h" | "24h" | "7d" | "30d";
const RANGE_MS: Record<LogRange, number> = { "1h": 3_600_000, "24h": 86_400_000, "7d": 7 * 86_400_000, "30d": 30 * 86_400_000 };

export interface LogFilters {
  range?: LogRange;
  from?: string;
  to?: string;
  key?: string;
  member?: string;
  model?: string;
  provider?: string;
  status?: number;
  failed?: boolean;
  error?: string;
  stream?: boolean;
  source?: "api" | "playground";
}

export function logsParams(f: LogFilters, now = Date.now()) {
  const custom = f.from && f.to ? { from: f.from, to: f.to } : null;
  const from = custom ? `${custom.from}T00:00:00Z` : new Date(now - RANGE_MS[f.range ?? "24h"]).toISOString();
  const to = custom ? endOfDay(custom.to) : undefined;
  return {
    from,
    ...(to ? { to } : {}),
    ...(f.key ? { key: f.key } : {}),
    ...(f.member?.startsWith("user:") ? { user: f.member.slice(5) } : {}),
    ...(f.member?.startsWith("sa:") ? { service_account: f.member.slice(3) } : {}),
    ...(f.model ? { model: f.model } : {}),
    ...(f.provider ? { provider: f.provider } : {}),
    ...(f.status ? { status: f.status } : {}),
    ...(f.failed ? { failed: true } : {}),
    ...(f.error ? { error_kind: f.error } : {}),
    ...(f.stream !== undefined ? { stream: f.stream } : {}),
    ...(f.source ? { source: f.source } : {}),
  };
}

export const logsInfiniteQuery = (org: string, project: string, filters: LogFilters) =>
  infiniteQueryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "list", filters],
    initialPageParam: undefined as { after: string; from: string } | undefined,
    queryFn: async ({ pageParam, signal }) => {
      const query = { ...logsParams(filters), ...pageParam };
      const page = data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs", { params: { path: { org, project }, query: { ...query, limit: 100 } }, signal }),
      );
      return { ...page, from: query.from };
    },
    getNextPageParam: (page) => {
      const after = nextPageParam(page);
      return after ? { after, from: page.from } : undefined;
    },
    refetchOnWindowFocus: refetchOnFocusWhileOnePage,
  });

export const newestLogsQuery = (org: string, project: string, filters: LogFilters) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "newest", filters],
    queryFn: async ({ signal }): Promise<LogView[]> =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs", {
          params: { path: { org, project }, query: { ...logsParams(filters), limit: 100 } },
          signal,
        }),
      ).data,
  });

export const logEntryQuery = (org: string, project: string, log: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "one", log],
    queryFn: async (): Promise<LogEntryView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/{log}", { params: { path: { org, project, log } } })),
    staleTime: 5 * 60_000,
  });
