import type { DateRangePreset } from "@logicplanes/kaeso-ui";
import { formatISODate, parseISODate } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";
import { PERIODS, type PeriodId } from "./period";

function periodName(id: PeriodId): string {
  switch (id) {
    case "today":
      return t("format.period.today", "Today");
    case "7d":
      return t("format.period.last7Days", "Last 7 days");
    case "30d":
      return t("format.period.last30Days", "Last 30 days");
    case "90d":
      return t("format.period.last90Days", "Last 90 days");
    case "6m":
      return t("format.period.last6Months", "Last 6 months");
    case "12m":
      return t("format.period.last12Months", "Last 12 months");
    default:
      return t("format.period.allTime", "All time");
  }
}

export const pickedDay = (d: Date) => formatISODate(d);

export function pickerDate(day: string): Date {
  const [y = 1970, m = 1, d = 1] = day.split("-").map(Number);
  return parseISODate(day) ?? new Date(y, m - 1, d);
}

export const afterToday = (d: Date, now = new Date()) => pickedDay(d) > apiToday(now);

const apiToday = (now = new Date()) => now.toISOString().slice(0, 10);

const DAY = 86_400_000;
const shiftDay = (day: string, days: number) => new Date(Date.parse(`${day}T00:00:00Z`) + days * DAY).toISOString().slice(0, 10);
const days = (from: string, to: string) => ({ from: pickerDate(from), to: pickerDate(to) });

const DAY_RANGES: { label: () => string; range: DateRangePreset["range"] }[] = [
  { label: () => t("format.period.today", "Today"), range: () => days(apiToday(), apiToday()) },
  { label: () => t("format.period.last7Days", "Last 7 days"), range: () => days(shiftDay(apiToday(), -6), apiToday()) },
  { label: () => t("format.period.last30Days", "Last 30 days"), range: () => days(shiftDay(apiToday(), -29), apiToday()) },
  { label: () => t("format.period.last90Days", "Last 90 days"), range: () => days(shiftDay(apiToday(), -89), apiToday()) },
  { label: () => t("format.period.thisMonth", "This month"), range: () => days(`${apiToday().slice(0, 8)}01`, apiToday()) },
  {
    label: () => t("format.period.lastMonth", "Last month"),
    range: () => {
      const end = shiftDay(`${apiToday().slice(0, 8)}01`, -1);
      return days(`${end.slice(0, 8)}01`, end);
    },
  },
];

export function dayPresets(): DateRangePreset[] {
  return DAY_RANGES.map((p) => ({ label: p.label(), range: p.range }));
}

export const DAY_PRESETS: DateRangePreset[] = DAY_RANGES.map((p) => ({
  get label() {
    return p.label();
  },
  range: p.range,
}));

const span = (r: { from: string; to: string }) => ({ from: pickerDate(r.from), to: pickerDate(r.to) });

export function periodPresets(days: (id: PeriodId) => { from: string; to: string }): (DateRangePreset & { id: PeriodId })[] {
  return PERIODS.map((id) => ({ id, label: periodName(id), range: () => span(days(id)) }));
}

export const pickedPeriod = (presets: (DateRangePreset & { id: PeriodId })[], id: string | null): PeriodId | undefined =>
  id === null ? undefined : presets.find((p) => p.id === id)?.id;
