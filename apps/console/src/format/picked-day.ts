import type { DateRangePreset } from "@logicplanes/kaeso-ui";
import { formatISODate, parseISODate } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";
import { PERIODS, shiftDay, utcDay, type PeriodId } from "./period";
import { dayKey } from "./zone";

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

export const afterToday = (d: Date, now = new Date()) => pickedDay(d) > dayKey(now);

export const afterUtcToday = (d: Date, now = new Date()) => pickedDay(d) > utcDay(now);

const days = (from: string, to: string) => ({ from: pickerDate(from), to: pickerDate(to) });

function dayPresets(today: () => string): DateRangePreset[] {
  const ranges: { label: () => string; range: DateRangePreset["range"] }[] = [
    { label: () => t("format.period.today", "Today"), range: () => days(today(), today()) },
    { label: () => t("format.period.last7Days", "Last 7 days"), range: () => days(shiftDay(today(), -6), today()) },
    { label: () => t("format.period.last30Days", "Last 30 days"), range: () => days(shiftDay(today(), -29), today()) },
    { label: () => t("format.period.last90Days", "Last 90 days"), range: () => days(shiftDay(today(), -89), today()) },
    { label: () => t("format.period.thisMonth", "This month"), range: () => days(`${today().slice(0, 8)}01`, today()) },
    {
      label: () => t("format.period.lastMonth", "Last month"),
      range: () => {
        const end = shiftDay(`${today().slice(0, 8)}01`, -1);
        return days(`${end.slice(0, 8)}01`, end);
      },
    },
  ];
  return ranges.map((p) => ({
    get label() {
      return p.label();
    },
    range: p.range,
  }));
}

export const DAY_PRESETS = dayPresets(() => dayKey(new Date()));

export const UTC_DAY_PRESETS = dayPresets(() => utcDay(new Date()));

const span = (r: { from: string; to: string }) => ({ from: pickerDate(r.from), to: pickerDate(r.to) });

export function periodPresets(days: (id: PeriodId) => { from: string; to: string }): (DateRangePreset & { id: PeriodId })[] {
  return PERIODS.map((id) => ({ id, label: periodName(id), range: () => span(days(id)) }));
}

export const pickedPeriod = (presets: (DateRangePreset & { id: PeriodId })[], id: string | null): PeriodId | undefined =>
  id === null ? undefined : presets.find((p) => p.id === id)?.id;
