import { t } from "~/i18n";
import { PERIODS, shiftDay, utcDay, type PeriodId, type PeriodSearch } from "./period";

function shiftMonths(day: string, months: number): string {
  const [y = 1970, m = 1, d = 1] = day.split("-").map(Number);
  const last = new Date(Date.UTC(y, m - 1 + months + 1, 0)).getUTCDate();
  return utcDay(new Date(Date.UTC(y, m - 1 + months, Math.min(d, last))));
}

const BACK: Record<Exclude<PeriodId, "all">, (to: string) => string> = {
  today: (to) => to,
  "7d": (to) => shiftDay(to, -6),
  "30d": (to) => shiftDay(to, -29),
  "90d": (to) => shiftDay(to, -89),
  "6m": (to) => shiftDay(shiftMonths(to, -6), 1),
  "12m": (to) => shiftDay(shiftMonths(to, -12), 1),
};

export function presetDays(id: PeriodId, now = new Date(), first?: string | null): { from: string; to: string } {
  const to = utcDay(now);
  return { from: id === "all" ? (first && first < to ? first : to) : BACK[id](to), to };
}

export function resolvePeriod(s: PeriodSearch, first?: string | null, now = new Date()): { from: string; to: string; period: PeriodId | undefined } {
  const known = PERIODS.find((p) => p === s.period);
  if (!known && s.from && s.to) return { from: s.from, to: s.to, period: undefined };
  const period = known ?? "30d";
  return { ...presetDays(period, now, first), period };
}

export function comparedWith(period: PeriodId | undefined, days: number): string | undefined {
  if (period === "all") return undefined;
  if (period === "6m" || period === "12m")
    return t("format.period.vsPriorMonths", "{count, plural, one {vs. prior # month} other {vs. prior # months}}", { count: period === "6m" ? 6 : 12 });
  return t("format.period.vsPriorDays", "{count, plural, one {vs. prior # day} other {vs. prior # days}}", { count: days });
}
