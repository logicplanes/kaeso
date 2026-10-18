import { formatLocale } from "@logicplanes/kaeso-ui/format";

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
  const tag = formatLocale() === "en" ? "en-GB" : formatLocale();
  try {
    return new Intl.DateTimeFormat(tag, {
      timeZone: zone,
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    }).format(at);
  } catch {
    return "";
  }
}
