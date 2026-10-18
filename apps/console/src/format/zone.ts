import { formatTimeZone } from "@logicplanes/kaeso-ui/format";

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
