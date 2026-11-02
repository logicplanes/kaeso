import { formatTimeZone } from "@logicplanes/kaeso-ui/format";
import { shiftDay } from "./period";

export function inZone(): { timeZone?: string } {
  const timeZone = formatTimeZone();
  return timeZone ? { timeZone } : {};
}

export function dayKey(d: Date): string {
  return new Intl.DateTimeFormat("en-CA", { year: "numeric", month: "2-digit", day: "2-digit", ...inZone() }).format(d);
}

export function yearOf(d: Date): string {
  return dayKey(d).slice(0, 4);
}

function offsetAt(at: number, timeZone: string): number {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone,
    hourCycle: "h23",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
    second: "numeric",
  }).formatToParts(at);
  const part = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((p) => p.type === type)?.value);
  const wall = Date.UTC(part("year"), part("month") - 1, part("day"), part("hour"), part("minute"), part("second"));
  return wall - (at - (at % 1000));
}

export function dayStart(day: string): string {
  const [year = 1970, month = 1, date = 1] = day.split("-").map(Number);
  const timeZone = formatTimeZone();
  if (!timeZone) return new Date(year, month - 1, date).toISOString();
  const wall = Date.UTC(year, month - 1, date);
  // The offset at the wall time itself can be an hour off across a change of offset, so take it again at the first guess.
  return new Date(wall - offsetAt(wall - offsetAt(wall, timeZone), timeZone)).toISOString();
}

export const dayEnd = (day: string): string => dayStart(shiftDay(day, 1));
