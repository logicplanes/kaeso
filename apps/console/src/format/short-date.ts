import { dateFormatter, formatDate } from "@logicplanes/kaeso-ui/format";
import { inZone, yearOf } from "./zone";

export function dayWithinYear(value: string, now: Date = new Date()): string {
  const day = new Date(value);
  if (yearOf(day) !== yearOf(now)) return formatDate(value);
  return dateFormatter({ day: "numeric", month: "short", ...inZone() })(day);
}
