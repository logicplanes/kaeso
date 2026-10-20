import { dateFormatter, formatLocale } from "@logicplanes/kaeso-ui/format";

export function browserTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}

export function timeZoneNames(current: string): string[] {
  const names = new Set(["UTC", ...Intl.supportedValuesOf("timeZone"), current]);
  return [...names].sort((a, b) => (a === "UTC" ? -1 : b === "UTC" ? 1 : a.localeCompare(b)));
}

export function offsetOf(zone: string, at: Date = new Date()): string {
  try {
    const name = new Intl.DateTimeFormat("en-GB", { timeZone: zone, timeZoneName: "shortOffset" }).format(at).replace(/^.* /, "");
    return name === "GMT" ? "GMT+0" : name;
  } catch {
    return zone;
  }
}

export function clockIn(zone: string, at: Date = new Date()): string {
  try {
    return dateFormatter({ timeZone: zone, weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23" })(at);
  } catch {
    return "";
  }
}

export function hourLabel(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

const MONDAY = Date.UTC(2026, 9, 19);

export function weekdayName(index: number): string {
  const tag = formatLocale() === "en" ? "en-GB" : formatLocale();
  return new Intl.DateTimeFormat(tag, { weekday: "long", timeZone: "UTC" }).format(new Date(MONDAY + index * 86_400_000));
}
