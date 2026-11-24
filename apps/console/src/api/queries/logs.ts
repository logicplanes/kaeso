import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { shiftDay } from "~/format/period";
import { dayStart } from "~/format/zone";
import { api, data } from "../client";
import type { ClientApp, LogEntryView, LogView, LoggedEndpoint, RequestFormat, ToolCallEntryView, ToolCallView } from "../generated/types";
import { nextPageParam, refetchOnFocusWhileOnePage, type Page } from "../paging";

export type LogRange = "1h" | "24h" | "7d" | "30d";
const RANGE_MS: Record<LogRange, number> = { "1h": 3_600_000, "24h": 86_400_000, "7d": 7 * 86_400_000, "30d": 30 * 86_400_000 };

export interface LogFilters {
  range?: LogRange;
  from?: string;
  to?: string;
  /** `from` and `to` are days in UTC, as Analytics counts them, not in the install's time zone. */
  utc?: boolean;
  key?: string;
  member?: string;
  model?: string;
  provider?: string;
  status?: number;
  failed?: boolean;
  error?: string;
  stream?: boolean;
  source?: "api" | "playground" | "observed";
  /** The source that reported the requests. */
  source_id?: string;
  ingress?: string;
  egress?: string;
  alias?: string;
  fell_back?: boolean;
  endpoint?: string;
  /** In the organization's currency, as a decimal string such as `0.5`. */
  min_cost?: string;
  /** The model or alias name exactly as the app sent it. */
  requested_model?: string;
  response_cache?: "hit" | "miss" | "bypass";
  min_duration_ms?: number;
  /** One of the tags the app sent in `kaeso-tags`. */
  tag?: string;
  /** Text the error message contains, in any case. */
  error_message?: string;
  /** The client app, as the log names it from the user agent. */
  client?: string;
  /** An action a guardrail took on the request. */
  guardrail_action?: "allow" | "block" | "replace";
}

/** Every endpoint the request log names, as the gateway's `endpoint` filter takes them. */
export const ENDPOINTS: readonly LoggedEndpoint[] = [
  "audio/speech",
  "audio/transcriptions",
  "audio/translations",
  "chat/completions",
  "decisions",
  "embeddings",
  "flows/image",
  "flows/image/completed",
  "images/edits",
  "images/generations",
  "images/variations",
  "messages",
  "messages/count_tokens",
  "models:batchEmbedContents",
  "models:countTokens",
  "models:embedContent",
  "models:generateContent",
  "models:streamGenerateContent",
  "music",
  "rerank",
  "responses",
  "responses/input_tokens",
  "sound-generation",
  "speech-to-text",
  "speech-to-text/realtime",
  "systemone",
  "text-to-dialogue",
  "text-to-dialogue/multi-stream-input",
  "text-to-dialogue/stream-input",
  "text-to-speech",
  "text-to-speech/multi-stream-input",
  "text-to-speech/stream-input",
  "videos",
  "videos/completed",
  "videos/remix",
  "voice-changer",
];

export function logsParams(f: LogFilters, now = Date.now()) {
  const custom = f.from && f.to ? { from: f.from, to: f.to } : null;
  const start = f.utc ? utcDayStart : dayStart;
  const from = custom ? start(custom.from) : new Date(now - RANGE_MS[f.range ?? "24h"]).toISOString();
  const to = custom ? start(shiftDay(custom.to, 1)) : undefined;
  return {
    from,
    ...(to ? { to } : {}),
    ...(f.key ? { key: f.key } : {}),
    ...(f.member?.startsWith("user:") ? { user: f.member.slice(5) } : {}),
    ...(f.member?.startsWith("sa:") ? { service_account: f.member.slice(3) } : {}),
    ...(f.model ? { model: f.model } : {}),
    ...(f.provider ? { provider: f.provider } : {}),
    ...(f.status ? { status: f.status } : {}),
    ...(f.failed !== undefined ? { failed: f.failed } : {}),
    ...(f.error ? { error_kind: f.error } : {}),
    ...(f.stream !== undefined ? { stream: f.stream } : {}),
    ...(f.source ? { source: f.source } : {}),
    ...(f.source_id ? { source_id: f.source_id } : {}),
    // The URL may hold any text; the gateway refuses a format it doesn't know.
    ...(f.ingress ? { ingress_dialect: f.ingress as RequestFormat } : {}),
    ...(f.egress ? { egress_dialect: f.egress as RequestFormat } : {}),
    ...(f.alias ? { alias: f.alias } : {}),
    ...(f.fell_back ? { fell_back: true } : {}),
    ...(f.endpoint ? { endpoint: f.endpoint as LoggedEndpoint } : {}),
    ...(f.min_cost ? { min_cost: f.min_cost } : {}),
    ...(f.requested_model ? { requested_model: f.requested_model } : {}),
    ...(f.response_cache ? { response_cache: f.response_cache } : {}),
    ...(f.min_duration_ms ? { min_duration_ms: f.min_duration_ms } : {}),
    ...(f.tag ? { tag: f.tag } : {}),
    ...(f.error_message ? { error_message: f.error_message } : {}),
    // The URL may hold any text; the gateway refuses a client it doesn't know.
    ...(f.client ? { client: f.client as ClientApp } : {}),
    ...(f.guardrail_action ? { guardrail_action: f.guardrail_action } : {}),
  };
}

/** The period's most common failures among the requests the filters keep. */
export const logErrorsQuery = (org: string, project: string, filters: LogFilters) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "errors", filters],
    queryFn: async ({ signal }) =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/errors", {
          params: { path: { org, project }, query: { ...logsParams(filters), limit: 10 } },
          signal,
        }),
      ).data,
  });

const utcDayStart = (day: string) => `${day}T00:00:00.000Z`;

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

/** The newest requests, those after `since` when given, asked for `rows` at a time; a view that shows a few asks for only those. */
export const newestLogsQuery = (org: string, project: string, filters: LogFilters, since?: string, rows = 100) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "newest", filters, since ?? null, rows],
    queryFn: ({ signal }): Promise<NewestLogs> => {
      const query = { ...logsParams(filters), ...(since ? { from: since } : {}), limit: rows };
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
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/{log}", {
          params: { path: { org, project, log } },
        }),
      ),
    staleTime: 5 * 60_000,
  });

export interface ToolCallFilters {
  range?: LogRange;
  from?: string;
  to?: string;
  utc?: boolean;
  key?: string;
  member?: string;
  server?: string;
  tool?: string;
  /** The source that reported the calls. */
  source_id?: string;
  status?: "ok" | "error" | "refused";
  error?: string;
}

/** What of a request log address the tool calls are filtered by: its period and party, and the `call_*` parameters. */
export interface ToolCallSearch {
  range?: LogRange;
  from?: string;
  to?: string;
  key?: string;
  member?: string;
  source_id?: string;
  call_server?: string;
  call_tool?: string;
  call_status?: "ok" | "error" | "refused";
  call_error?: string;
}

/** The tool calls' filters for an address; the page and the route's loader both read them here, so they ask for the same list. */
export function toolCallFilters(search: ToolCallSearch): ToolCallFilters {
  return {
    ...(search.range ? { range: search.range } : {}),
    ...(search.from && search.to ? { from: search.from, to: search.to } : {}),
    ...(search.key ? { key: search.key } : {}),
    ...(search.member ? { member: search.member } : {}),
    ...(search.call_server ? { server: search.call_server } : {}),
    ...(search.call_tool ? { tool: search.call_tool } : {}),
    ...(search.source_id ? { source_id: search.source_id } : {}),
    ...(search.call_status ? { status: search.call_status } : {}),
    ...(search.call_error ? { error: search.call_error } : {}),
  };
}

export function toolCallParams(f: ToolCallFilters, now = Date.now()) {
  const { from, to, key, user, service_account } = logsParams(
    {
      ...(f.range ? { range: f.range } : {}),
      ...(f.from && f.to ? { from: f.from, to: f.to } : {}),
      ...(f.utc ? { utc: true } : {}),
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
    ...(f.source_id ? { source_id: f.source_id } : {}),
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
