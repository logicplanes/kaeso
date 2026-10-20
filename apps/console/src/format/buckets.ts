import { locale, t } from "~/i18n";
import { dayFormat, dayLabel, daysBetween, utcDay } from "./period";

export type Granularity = "day" | "week" | "month";

export function granularity(from: string, to: string): Granularity {
  const days = (Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000 + 1;
  return days <= 92 ? "day" : days <= 371 ? "week" : "month";
}

const DAY = 86_400_000;
const at = (day: string) => Date.parse(`${day}T00:00:00Z`);
export const mondayOf = (day: string) => utcDay(new Date(at(day) - ((new Date(at(day)).getUTCDay() + 6) % 7) * DAY));

export function bucketsBetween(from: string, to: string, g: Granularity): string[] {
  if (g === "day") return daysBetween(from, to);
  const out: string[] = [];
  if (g === "week") {
    for (let t = at(mondayOf(from)); t <= at(to); t += 7 * DAY) out.push(utcDay(new Date(t)));
    return out;
  }
  const [y = 1970, m = 1] = from.split("-").map(Number);
  for (let i = 0; ; i++) {
    const key = utcDay(new Date(Date.UTC(y, m - 1 + i, 1)));
    if (key > to) return out;
    out.push(key);
  }
}

const month = dayFormat("en-US", { month: "short" });
const dayYearName = dayFormat("en-US", { month: "short", day: "numeric", year: "numeric" });
const dayMonthName = dayFormat("en-GB", { day: "numeric", month: "short" });
const dayMonthYearName = dayFormat("en-GB", { day: "numeric", month: "short", year: "numeric" });
const monthYearName = dayFormat("en-US", { month: "short", year: "numeric" });
const yearName = dayFormat("en-US", { year: "numeric" });
const english = () => locale() === "en";
const dayMonth = (day: string) => (english() ? `${Number(day.slice(8, 10))} ${month(day)}` : dayMonthName(day));
const dayMonthYear = (day: string) => (english() ? `${dayMonth(day)} ${day.slice(0, 4)}` : dayMonthYearName(day));
const monthYear = (day: string) => (english() ? `${month(day)} ${day.slice(0, 4)}` : monthYearName(day));

function opensYear(key: string, g: Granularity): boolean {
  if (key.slice(5, 7) !== "01") return false;
  const day = Number(key.slice(8, 10));
  return g === "day" ? day === 1 : g === "week" ? day <= 7 : true;
}

const dayName = (day: string, withYear = false) => (withYear || opensYear(day, "day") ? dayYearName(day) : dayLabel(day));
const weekName = (day: string, withYear = false) => (withYear || opensYear(day, "week") ? dayMonthYear(day) : dayMonth(day));

export const spansYears = (from: string, to: string, g: Granularity) => (bucketsBetween(from, to, g)[0] ?? from).slice(0, 4) !== to.slice(0, 4);

export function bucketLabel(key: string, g: Granularity, withYear = false): string {
  if (g === "day") return dayName(key, withYear);
  return g === "week" ? t("format.bucket.weekOf", "Week of {date}", { date: weekName(key, withYear) }) : monthYear(key);
}

const lastDay = (key: string, g: Granularity) => {
  if (g === "day") return key;
  if (g === "week") return utcDay(new Date(at(key) + 6 * DAY));
  const [y = 1970, m = 1] = key.split("-").map(Number);
  return utcDay(new Date(Date.UTC(y, m, 0)));
};

export function partialBuckets(from: string, to: string, g: Granularity, today: string): { first: boolean; last: boolean; ongoing: boolean } {
  const keys = bucketsBetween(from, to, g);
  const last = keys[keys.length - 1];
  return { first: (keys[0] ?? from) < from, last: last !== undefined && lastDay(last, g) > to, ongoing: to >= today };
}

export function bucketAxisLabel(key: string, g: Granularity): string {
  if (g === "day") return dayName(key);
  if (g === "week") return weekName(key);
  return opensYear(key, g) ? yearName(key) : month(key);
}
