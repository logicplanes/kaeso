import { formatUSD } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { bucketsBetween, type Granularity } from "~/format/buckets";
import { t } from "~/i18n";
import { totalTokens } from "./tokens";

export function sumUSD(values: (string | null | undefined)[]): string {
  const micros = values.reduce((acc, v) => acc + (v ? Math.round(Number(v) * 1e6) : 0), 0);
  return (micros / 1e6).toFixed(6).replace(/0+$/, "").replace(/\.$/, ".00");
}

export function periodUSD(amount: string | null | undefined): string {
  if (!amount || !/[1-9]/.test(amount)) return formatUSD("0", { decimals: 2 });
  const n = Number(amount);
  if (n > 0 && n < 0.005) return t("kaeso.usage.lessThan", "<{amount}", { amount: formatUSD("0.01", { decimals: 2 }) });
  return formatUSD(amount, { decimals: 2 });
}

export interface UsageTotals {
  requests: number;
  errors: number;
  tokens: number;
  cost: string | null;
  toolCalls: number;
}

export function totals(rows: UsageRow[]): UsageTotals {
  const withCost = rows.some((r) => r.cost !== null && r.cost !== undefined);
  return {
    requests: rows.reduce((a, r) => a + r.requests, 0),
    errors: rows.reduce((a, r) => a + r.errors, 0),
    tokens: rows.reduce((a, r) => a + totalTokens(r), 0),
    cost: withCost ? sumUSD(rows.map((r) => r.cost)) : null,
    toolCalls: rows.reduce((a, r) => a + r.tool_calls, 0),
  };
}

export function tileUSD(v: string): string {
  if (!(Number(v) > 0 && Number(v) < 0.01)) return formatUSD(v);
  const rounded = Number(Number(v).toPrecision(2));
  const fixed = rounded.toFixed(1 - Math.floor(Math.log10(rounded))).replace(/0+$/, "");
  return formatUSD(fixed, { decimals: fixed.length - "0.".length });
}
export function byBucket(rows: UsageRow[], from: string, to: string, g: Granularity = "day") {
  const map = new Map(rows.map((r) => [r.key, r]));
  return bucketsBetween(from, to, g).map((day) => ({ day, row: map.get(day) }));
}
