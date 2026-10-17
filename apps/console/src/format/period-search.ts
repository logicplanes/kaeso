export const utcDay = (d: Date) => d.toISOString().slice(0, 10);

export function parseDay(value: unknown): string | undefined {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return undefined;
  const t = Date.parse(`${value}T00:00:00Z`);
  return !Number.isNaN(t) && utcDay(new Date(t)) === value ? value : undefined;
}

export const PERIODS = ["today", "7d", "30d", "90d", "6m", "12m", "all"] as const;
export type PeriodId = (typeof PERIODS)[number];

export type PeriodSearch = { period?: PeriodId | undefined; from?: string | undefined; to?: string | undefined };

export function periodSearch(s: Record<string, unknown>): PeriodSearch {
  const period = PERIODS.find((p) => p === s.period);
  if (period) return { period };
  const from = parseDay(s.from);
  const to = parseDay(s.to);
  return from && to && from <= to ? { from, to } : {};
}
