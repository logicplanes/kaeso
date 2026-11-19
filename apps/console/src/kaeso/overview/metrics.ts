import { formatNumber, formatPercent, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { shiftDay } from "~/format/period";
import { t } from "~/i18n";
import { totalTokens } from "~/kaeso/tokens";
import type { Money } from "~/format/currency";
import { totals } from "~/kaeso/usage-panels";

export type MetricId = "requests" | "tokens" | "cost" | "errors" | "tools" | "cache";
export type Dimension = "project" | "model" | "key" | "member";

export interface Metric {
  id: MetricId;
  label: () => string;
  color: string;
  of: (r: UsageRow | undefined) => number;
  rank: (r: UsageRow) => number;
  /** `money` writes amounts of spend in the organization's currency. */
  format: (v: number, money: Money) => string;
  formatRank: (v: number, money: Money) => string;
  better: "up" | "down" | "none";
}

export function cacheHit(rows: UsageRow[]): number | null {
  const read = rows.reduce((a, r) => a + r.cache_read_tokens, 0);
  const all = rows.reduce((a, r) => a + r.input_tokens + r.cache_read_tokens + r.cache_write_tokens, 0);
  return all ? read / all : null;
}

// The chart palette's tokens by name: importing VIZ would load the chart code into every page that shows a metric.
const viz = (i: number) => `var(--k-viz-${i + 1})`;
const costOf = (r: UsageRow | undefined) => Number(r?.converted_cost ?? 0);
const tokens = (v: number) => formatTokens(v);

export const METRICS: Record<MetricId, Metric> = {
  requests: {
    id: "requests",
    label: () => t("common.requests", "Requests"),
    color: viz(0),
    of: (r) => r?.requests ?? 0,
    rank: (r) => r.requests,
    format: (v) => formatNumber(v),
    formatRank: (v) => formatNumber(v),
    better: "none",
  },
  tokens: {
    id: "tokens",
    label: () => t("common.tokens", "Tokens"),
    color: viz(1),
    of: (r) => (r ? totalTokens(r) : 0),
    rank: (r) => totalTokens(r),
    format: tokens,
    formatRank: tokens,
    better: "none",
  },
  cost: {
    id: "cost",
    label: () => t("common.cost", "Cost"),
    color: viz(2),
    of: costOf,
    rank: (r) => costOf(r),
    format: (v, money) => money.tile(v.toFixed(6)),
    formatRank: (v, money) => money.period(v.toFixed(6)),
    better: "none",
  },
  errors: {
    id: "errors",
    label: () => t("kaeso.overview.errorRate", "Error rate"),
    color: viz(3),
    of: (r) => (r && r.requests ? r.errors / r.requests : 0),
    rank: (r) => r.errors,
    format: (v) => formatPercent(v),
    formatRank: (v) => formatNumber(v),
    better: "down",
  },
  tools: {
    id: "tools",
    label: () => t("kaeso.usage.toolCalls", "Tool calls"),
    color: viz(4),
    of: (r) => r?.tool_calls ?? 0,
    rank: (r) => r.tool_calls,
    format: (v) => formatNumber(v),
    formatRank: (v) => formatNumber(v),
    better: "none",
  },
  cache: {
    id: "cache",
    label: () => t("kaeso.overview.cacheHit", "Cache hit"),
    color: viz(5),
    of: (r) => cacheHit(r ? [r] : []) ?? 0,
    rank: (r) => r.cache_read_tokens,
    format: (v) => formatPercent(v, 0),
    formatRank: tokens,
    better: "up",
  },
};

export const change = (now: number, before: number | undefined) => (before ? (now - before) / before : undefined);

export function metricTotal(id: MetricId, rows: UsageRow[]): number {
  const s = totals(rows);
  switch (id) {
    case "requests":
      return s.requests;
    case "tokens":
      return s.tokens;
    case "cost":
      return Number(s.cost ?? 0);
    case "errors":
      return s.requests ? s.errors / s.requests : 0;
    case "tools":
      return s.toolCalls;
    case "cache":
      return cacheHit(rows) ?? 0;
  }
}

/** Spend so far this month and where it lands if the last seven full days keep their pace. */
// The pace takes the seven days before `today`, which reach into the previous month during the
// first week; the rows the forecast reads start there.
export function forecastFrom(today: string): string {
  const monthStart = `${today.slice(0, 8)}01`;
  const weekAgo = shiftDay(today, -7);
  return weekAgo < monthStart ? weekAgo : monthStart;
}

export function forecast(rows: UsageRow[], from: string, to: string) {
  const monthStart = `${to.slice(0, 8)}01`;
  if (from > forecastFrom(to)) return undefined;
  const byDay = new Map(rows.map((r) => [r.key, Number(r.converted_cost ?? 0)]));
  const [y = 1970, m = 1, d = 1] = to.split("-").map(Number);
  const daysInMonth = new Date(Date.UTC(y, m, 0)).getUTCDate();
  let soFar = 0;
  for (let day = monthStart; day <= to; day = shiftDay(day, 1)) soFar += byDay.get(day) ?? 0;
  let week = 0;
  for (let i = 1; i <= 7; i++) week += byDay.get(shiftDay(to, -i)) ?? 0;
  const pace = week / 7;
  const today = byDay.get(to) ?? 0;
  return { soFar, projected: soFar - today + pace * (daysInMonth - d + 1), month: monthStart };
}
