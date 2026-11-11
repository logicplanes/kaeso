import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";

export const TOOL = "show_usage_stats";
export const METRICS = ["requests", "tokens", "cost", "latency", "errors"] as const;
export const RANGES = ["24h", "7d", "30d"] as const;
export const GROUPS = ["model", "provider", "key"] as const;
export type Metric = (typeof METRICS)[number];
export type Range = (typeof RANGES)[number];
export type Group = (typeof GROUPS)[number];
export type Value = number | string | null;

export interface Point {
  start: string;
  value: Value;
}

export interface GroupRow {
  key: string;
  label: string;
  value: Value;
  requests: number;
}

export interface Stats {
  metric: Metric;
  range: Range;
  group_by: Group;
  scope: { org: { id: string; name: string }; project: { id: string; name: string } | null };
  from: string;
  to: string;
  step: "hour" | "day";
  totals: {
    requests: number;
    tokens: number;
    cost: string | null;
    errors: number;
    latency_p95_ms: number | null;
    latency_p50_ms: number | null;
  };
  series: Point[];
  groups: GroupRow[];
  console_url: string;
}

export type Failure = { kind: "rights"; message: string; approveUrl: string | null } | { kind: "error"; message: string } | { kind: "cancelled" };

export type Outcome = { stats: Stats } | { failure: Failure };

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const oneOf = <T extends string>(all: readonly T[], v: unknown): v is T => typeof v === "string" && (all as readonly string[]).includes(v);

// Only http(s) leaves the view: a link from a result is opened through the host, never assigned to the page.
export function safeUrl(v: unknown): string | null {
  if (typeof v !== "string") return null;
  try {
    const url = new URL(v);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
  } catch {
    return null;
  }
}

function text(result: CallToolResult): string {
  for (const block of result.content ?? []) if (block.type === "text" && block.text) return block.text;
  return "";
}

export function isStats(v: unknown): v is Stats {
  return (
    isObject(v) &&
    oneOf(METRICS, v.metric) &&
    oneOf(RANGES, v.range) &&
    oneOf(GROUPS, v.group_by) &&
    isObject(v.totals) &&
    isObject(v.scope) &&
    Array.isArray(v.series) &&
    Array.isArray(v.groups) &&
    (v.step === "hour" || v.step === "day")
  );
}

export function outcomeOf(result: CallToolResult): Outcome {
  const structured = result.structuredContent;
  if (result.isError) {
    const error = isObject(structured) && isObject(structured.error) ? structured.error : null;
    if (error?.type === "insufficient_rights")
      return { failure: { kind: "rights", message: typeof error.message === "string" ? error.message : text(result), approveUrl: safeUrl(error.approve_url) } };
    return { failure: { kind: "error", message: text(result) } };
  }
  if (isStats(structured)) return { stats: structured };
  return { failure: { kind: "error", message: text(result) } };
}

export interface Ask {
  metric?: Metric | undefined;
  range?: Range | undefined;
  group_by?: Group | undefined;
}

export function askOf(args: Record<string, unknown> | undefined): Ask {
  const a = args ?? {};
  return {
    metric: oneOf(METRICS, a.metric) ? a.metric : undefined,
    range: oneOf(RANGES, a.range) ? a.range : undefined,
    group_by: oneOf(GROUPS, a.group_by) ? a.group_by : undefined,
  };
}

export function numeric(v: Value): number | null {
  if (v === null) return null;
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : null;
}
