import { formatDate, formatLocale } from "@logicplanes/kaeso-ui/format";
import { inZone, yearOf } from "./zone";

const formats = new Map<string, Intl.DateTimeFormat>();

export function dayWithinYear(value: string, now: Date = new Date()): string {
  const day = new Date(value);
  if (yearOf(day) !== yearOf(now)) return formatDate(value);
  const tag = formatLocale() === "en" ? "en-GB" : formatLocale();
  const zone = inZone();
  const key = `${tag}|${zone.timeZone ?? ""}`;
  let format = formats.get(key);
  if (!format) formats.set(key, (format = new Intl.DateTimeFormat(tag, { day: "numeric", month: "short", ...zone })));
  return format.format(day);
}
