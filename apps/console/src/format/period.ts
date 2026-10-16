import { locale } from "~/i18n";

export const utcDay = (d: Date) => d.toISOString().slice(0, 10);

export function daysBetween(from: string, to: string): string[] {
  const out: string[] = [];
  for (let t = Date.parse(`${from}T00:00:00Z`); t <= Date.parse(`${to}T00:00:00Z`); t += 86_400_000) out.push(utcDay(new Date(t)));
  return out;
}

export function dayFormat(english: "en-US" | "en-GB", options: Intl.DateTimeFormatOptions): (day: string) => string {
  const made = new Map<string, Intl.DateTimeFormat>();
  return (day) => {
    const tag = locale() === "en" ? english : locale();
    let f = made.get(tag);
    if (!f) made.set(tag, (f = new Intl.DateTimeFormat(tag, { ...options, timeZone: "UTC" })));
    return f.format(new Date(`${day}T00:00:00Z`));
  };
}

export const dayLabel = dayFormat("en-US", { month: "short", day: "numeric" });

export function parseDay(value: unknown): string | undefined {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return undefined;
  const t = Date.parse(`${value}T00:00:00Z`);
  return !Number.isNaN(t) && utcDay(new Date(t)) === value ? value : undefined;
}

export const monthLabel = dayFormat("en-GB", { month: "short", year: "numeric" });

export const fullDayLabel = dayFormat("en-GB", { day: "numeric", month: "short", year: "numeric" });

export function previousPeriod(from: string, to: string): { from: string; to: string; days: number } {
  const days = daysBetween(from, to).length;
  const end = Date.parse(`${from}T00:00:00Z`) - 86_400_000;
  return { from: utcDay(new Date(end - (days - 1) * 86_400_000)), to: utcDay(new Date(end)), days };
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
