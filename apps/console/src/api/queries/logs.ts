import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { dayEnd, dayStart } from "~/format/zone";
import { api, data } from "../client";
import type { LogEntryView, LogView, ToolCallEntryView, ToolCallView } from "../generated/types";
import { nextPageParam, refetchOnFocusWhileOnePage, type Page } from "../paging";

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
  ingress?: string;
  egress?: string;
}

export function logsParams(f: LogFilters, now = Date.now()) {
  const custom = f.from && f.to ? { from: f.from, to: f.to } : null;
  const from = custom ? dayStart(custom.from) : new Date(now - RANGE_MS[f.range ?? "24h"]).toISOString();
  const to = custom ? dayEnd(custom.to) : undefined;
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
    ...(f.ingress ? { ingress_dialect: f.ingress } : {}),
    ...(f.egress ? { egress_dialect: f.egress } : {}),
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

export const LIVE_PAGES = 5;

export interface Newest<T> {
  rows: T[];
  complete: boolean;
}
export type NewestLogs = Newest<LogView>;

async function newestRows<T>(fetchPage: (after: string | undefined) => Promise<Page<T>>, since: string | undefined): Promise<Newest<T>> {
  const rows: T[] = [];
  let after: string | undefined;
  for (let pages = 0; pages < LIVE_PAGES; pages++) {
    const page = await fetchPage(after);
    rows.push(...page.data);
    after = nextPageParam(page);
    if (!after || !since) return { rows, complete: true };
  }
  return { rows, complete: false };
}

export const newestLogsQuery = (org: string, project: string, filters: LogFilters, since?: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "newest", filters, since ?? null],
    queryFn: ({ signal }): Promise<NewestLogs> => {
      const query = { ...logsParams(filters), ...(since ? { from: since } : {}), limit: 100 };
      return newestRows(
        async (after) =>
          data(
            await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs", {
              params: { path: { org, project }, query: { ...query, ...(after ? { after } : {}) } },
              signal,
            }),
          ),
        since,
      );
    },
    gcTime: 0,
  });

export const logEntryQuery = (org: string, project: string, log: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "one", log],
    queryFn: async (): Promise<LogEntryView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/{log}", { params: { path: { org, project, log } } })),
    staleTime: 5 * 60_000,
  });

export interface ToolCallFilters {
  range?: LogRange;
  from?: string;
  to?: string;
  key?: string;
  member?: string;
  server?: string;
  tool?: string;
  status?: "ok" | "error" | "refused";
  error?: string;
}

export function toolCallParams(f: ToolCallFilters, now = Date.now()) {
  const { from, to, key, user, service_account } = logsParams(
    {
      ...(f.range ? { range: f.range } : {}),
      ...(f.from && f.to ? { from: f.from, to: f.to } : {}),
      ...(f.key ? { key: f.key } : {}),
      ...(f.member ? { member: f.member } : {}),
    },
    now,
  );
  return {
    from,
    ...(to ? { to } : {}),
    ...(key ? { key } : {}),
    ...(user ? { user } : {}),
    ...(service_account ? { service_account } : {}),
    ...(f.server ? { server: f.server } : {}),
    ...(f.tool ? { tool: f.tool } : {}),
    ...(f.status ? { status: f.status } : {}),
    ...(f.error ? { error_type: f.error } : {}),
  };
}

export const toolCallsInfiniteQuery = (org: string, project: string, filters: ToolCallFilters) =>
  infiniteQueryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "tool-calls", filters],
    initialPageParam: undefined as { after: string; from: string } | undefined,
    queryFn: async ({ pageParam, signal }) => {
      const query = { ...toolCallParams(filters), ...pageParam };
      const page = data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/tool-calls", {
          params: { path: { org, project }, query: { ...query, limit: 100 } },
          signal,
        }),
      );
      return { ...page, from: query.from };
    },
    getNextPageParam: (page) => {
      const after = nextPageParam(page);
      return after ? { after, from: page.from } : undefined;
    },
    refetchOnWindowFocus: refetchOnFocusWhileOnePage,
  });

export const newestToolCallsQuery = (org: string, project: string, filters: ToolCallFilters, since?: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "tool-calls", "newest", filters, since ?? null],
    queryFn: ({ signal }): Promise<Newest<ToolCallView>> => {
      const query = { ...toolCallParams(filters), ...(since ? { from: since } : {}), limit: 100 };
      return newestRows(
        async (after) =>
          data(
            await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/tool-calls", {
              params: { path: { org, project }, query: { ...query, ...(after ? { after } : {}) } },
              signal,
            }),
          ),
        since,
      );
    },
    gcTime: 0,
  });

export const toolCallQuery = (org: string, project: string, call: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "tool-call", call],
    queryFn: async ({ signal }): Promise<ToolCallEntryView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/tool-calls/{call}", { params: { path: { org, project, call } }, signal })),
  });
