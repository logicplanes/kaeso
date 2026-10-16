import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import { locale, t } from "~/i18n";

export function seconds(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${formatNumber(ms / 1000, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
  return formatDuration(ms);
}

export const dateLocale = (): string => (locale() === "en" ? "en-GB" : locale());

export function startedLabel(iso: string, now = new Date()): string {
  const d = new Date(iso);
  const when =
    d.toDateString() === now.toDateString()
      ? new Intl.DateTimeFormat(dateLocale(), { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" }).format(d)
      : new Intl.DateTimeFormat(dateLocale(), { day: "numeric", month: "short" }).format(d);
  return t("logs.inspector.started", "Started {when}", { when });
}
