import type { SortingState } from "@logicplanes/kaeso-ui";
import type { UsageRow } from "~/api/generated/types";
import { totalTokens } from "./tokens";

// The number each sortable usage column orders by; null for what isn't known, which sorts last.
const SORT_VALUE: Record<string, (r: UsageRow) => number | null> = {
  requests: (r) => r.requests,
  errors: (r) => r.errors,
  tokens: (r) => totalTokens(r),
  cache: (r) => r.cache_hit_rate ?? null,
  // Cost is a decimal string; one the gateway can't price has none.
  cost: (r) => (r.cost ? Number(r.cost) : null),
};

/** Whether a usage table's column sorts: the numeric ones. */
export const sortsUsage = (id: string): boolean => id in SORT_VALUE;

/** The rows in the table's order: as given, or by the sorted column with unknown values last. */
export function sortUsage(rows: UsageRow[], sorting: SortingState): UsageRow[] {
  const by = sorting[0];
  const value = by ? SORT_VALUE[by.id] : undefined;
  if (!by || !value) return rows;
  return [...rows].sort((a, b) => {
    const x = value(a);
    const y = value(b);
    if (x === y) return 0;
    if (x === null) return 1;
    if (y === null) return -1;
    return by.desc ? y - x : x - y;
  });
}

export interface CsvColumn {
  header: string;
  value: (r: UsageRow) => string | number | null | undefined;
}

// A spreadsheet runs a cell that starts like a formula, so a name such as `=SUM(…)` is kept as text.
const cell = (v: string | number | null | undefined): string => {
  if (v === null || v === undefined) return "";
  if (typeof v === "number") return String(v);
  const text = /^[=+\-@\t\r]/.test(v) ? `'${v}` : v;
  return /[",\n\r]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
};

/** The rows as CSV, one line per row in the given order, with a header line. */
export function usageCsv(columns: readonly CsvColumn[], rows: readonly UsageRow[]): string {
  const lines = [columns.map((c) => cell(c.header)), ...rows.map((r) => columns.map((c) => cell(c.value(r))))];
  return `${lines.map((l) => l.join(",")).join("\r\n")}\r\n`;
}

/** Saves the CSV through the browser; nothing is sent anywhere. */
export function saveCsv(name: string, text: string): void {
  const href = URL.createObjectURL(new Blob([text], { type: "text/csv;charset=utf-8" }));
  const a = document.createElement("a");
  a.href = href;
  a.download = name;
  a.click();
  URL.revokeObjectURL(href);
}
