import type { UsageRow } from "~/api/generated/types";
import { sumAmounts } from "~/format/currency";
import { bucketsBetween, type Granularity } from "~/format/buckets";
import { totalTokens } from "./tokens";

export interface UsageTotals {
  requests: number;
  errors: number;
  tokens: number;
  /** In the organization's currency. */
  cost: string | null;
  toolCalls: number;
  /** Requests whose cost is unknown, which `cost` leaves out. */
  unpriced: number;
}

export function totals(rows: UsageRow[]): UsageTotals {
  const withCost = rows.some((r) => r.converted_cost !== null && r.converted_cost !== undefined);
  return {
    requests: rows.reduce((a, r) => a + r.requests, 0),
    errors: rows.reduce((a, r) => a + r.errors, 0),
    tokens: rows.reduce((a, r) => a + totalTokens(r), 0),
    cost: withCost ? sumAmounts(rows.map((r) => r.converted_cost)) : null,
    toolCalls: rows.reduce((a, r) => a + r.tool_calls, 0),
    unpriced: rows.reduce((a, r) => a + r.unpriced, 0),
  };
}

export function byBucket(rows: UsageRow[], from: string, to: string, g: Granularity = "day") {
  const map = new Map(rows.map((r) => [r.key, r]));
  return bucketsBetween(from, to, g).map((day) => ({ day, row: map.get(day) }));
}
