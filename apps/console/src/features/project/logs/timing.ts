import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import { locale, t } from "~/i18n";
import { dayKey, inZone } from "~/format/zone";

export function seconds(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${formatNumber(ms / 1000, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
  return formatDuration(ms);
}

export const dateLocale = (): string => (locale() === "en" ? "en-GB" : locale());

export function startedLabel(iso: string, now = new Date()): string {
  const d = new Date(iso);
  const when =
    dayKey(d) === dayKey(now)
      ? new Intl.DateTimeFormat(dateLocale(), { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23", ...inZone() }).format(d)
      : new Intl.DateTimeFormat(dateLocale(), { day: "numeric", month: "short", ...inZone() }).format(d);
  return t("logs.inspector.started", "Started {when}", { when });
}
