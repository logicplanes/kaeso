import { locale } from "~/i18n";
import { utcDay } from "./period-search";

export { parseDay, PERIODS, periodSearch, utcDay, type PeriodId, type PeriodSearch } from "./period-search";

export const shiftDay = (day: string, days: number) => utcDay(new Date(Date.parse(`${day}T00:00:00Z`) + days * 86_400_000));

export const endOfDay = (day: string) => new Date(Date.parse(`${day}T00:00:00Z`) + 86_400_000).toISOString();

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

export const monthLabel = dayFormat("en-GB", { month: "short", year: "numeric" });

export const fullDayLabel = dayFormat("en-GB", { day: "numeric", month: "short", year: "numeric" });

export function previousPeriod(from: string, to: string): { from: string; to: string; days: number } {
  const days = daysBetween(from, to).length;
  const end = Date.parse(`${from}T00:00:00Z`) - 86_400_000;
  return { from: utcDay(new Date(end - (days - 1) * 86_400_000)), to: utcDay(new Date(end)), days };
}
