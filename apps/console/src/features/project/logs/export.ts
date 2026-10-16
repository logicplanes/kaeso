import type { ExportFile } from "~/api/download";
import { logsParams, type LogFilters } from "~/api/queries/logs";
import { errorKindLabel, providerLabel } from "~/format/labels";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

export type ExportFormat = "csv" | "jsonl" | "content";

const PERIOD = { "1h": "hour", "24h": "day", "7d": "week", "30d": "month" } as const;

export function exportSummary(f: LogFilters): string {
  const period = f.from && f.to ? (f.from === f.to ? "date" : "span") : PERIOD[f.range ?? "24h"];
  const days = f.from && f.to ? { day: fullDayLabel(f.from), from: fullDayLabel(f.from), to: fullDayLabel(f.to) } : {};
  const count = exportFilters(f).length;
  return count
    ? t(
        "logs.export.summaryFiltered",
        "{period, select, hour {Every request from the last hour that matches {count, plural, one {this filter} other {these filters}}, newest first.} day {Every request from the last 24 hours that matches {count, plural, one {this filter} other {these filters}}, newest first.} week {Every request from the last 7 days that matches {count, plural, one {this filter} other {these filters}}, newest first.} month {Every request from the last 30 days that matches {count, plural, one {this filter} other {these filters}}, newest first.} date {Every request on {day} that matches {count, plural, one {this filter} other {these filters}}, newest first.} other {Every request from {from} to {to} that matches {count, plural, one {this filter} other {these filters}}, newest first.}}",
        { period, count, ...days },
      )
    : t(
        "logs.export.summary",
        "{period, select, hour {Every request from the last hour, newest first.} day {Every request from the last 24 hours, newest first.} week {Every request from the last 7 days, newest first.} month {Every request from the last 30 days, newest first.} date {Every request on {day}, newest first.} other {Every request from {from} to {to}, newest first.}}",
        { period, ...days },
      );
}

export type ExportNames = { key?: string | undefined; member?: string | undefined; model?: string | undefined };

export function exportFilters(f: LogFilters, names: ExportNames = {}): { label: string; value: string }[] {
  const filters: { label: string; value: string | false | undefined }[] = [
    { label: t("logs.filter.key", "Key"), value: f.key && (names.key ?? f.key) },
    { label: t("logs.filter.member", "Member"), value: f.member && (names.member ?? f.member.replace(/^(user|sa):/, "")) },
    { label: common.model(), value: f.model && (names.model ?? f.model) },
    { label: common.provider(), value: f.provider && providerLabel(f.provider) },
    { label: common.status(), value: f.status !== undefined && String(f.status) },
    { label: t("logs.filter.failed", "Failed only"), value: !!f.failed && common.yes() },
    { label: t("logs.filter.error", "Error kind"), value: f.error && errorKindLabel(f.error) },
    {
      label: t("logs.filter.streaming", "Streaming"),
      value: f.stream !== undefined && (f.stream ? t("logs.filter.streamed", "Streamed") : t("logs.filter.notStreamed", "Not streamed")),
    },
    { label: t("logs.filter.source", "Source"), value: f.source && (f.source === "api" ? "API" : t("logs.source.playground", "Playground")) },
  ];
  return filters.flatMap(({ label, value }) => (value ? [{ label, value }] : []));
}

export const JSON_LINES = "JSON Lines";

const FILE: Record<"csv" | "jsonl", Omit<ExportFile, "name">> = {
  csv: { description: "CSV", mime: "text/csv", extension: "csv" },
  jsonl: { description: JSON_LINES, mime: "application/x-ndjson", extension: "jsonl" },
};

export function exportRequest(org: string, project: string, f: LogFilters, format: ExportFormat, now = Date.now()) {
  const params = logsParams(f, now);
  const span = { ...params, ...(params.to ? {} : { to: new Date(now).toISOString() }) };
  const query = (extra: Record<string, string | number | boolean>) =>
    new URLSearchParams(Object.entries({ ...span, ...extra }).map(([k, v]) => [k, String(v)])).toString();
  const base = `/platform/v1/orgs/${org}/projects/${project}/logs`;
  const type = format === "csv" ? "csv" : "jsonl";
  return {
    url: `${base}/export?${query({ format: type, ...(format === "content" ? { include_content: true } : {}) })}`,
    check: `${base}?${query({ limit: 1 })}`,
    file: { name: `kaeso-logs-${project}.${FILE[type].extension}`, ...FILE[type] },
  };
}
