import type { ExportFile } from "~/api/download";
import { logsParams, type LogFilters } from "~/api/queries/logs";
import { errorKindLabel, providerLabel } from "~/format/labels";
import { fullDayLabel } from "~/format/period";
import { locale, t } from "~/i18n";

export type ExportFormat = "csv" | "jsonl" | "content";

const period = (f: LogFilters): string => {
  if (f.from && f.to)
    return f.from === f.to
      ? t("logs.export.onDay", "on {day}", { day: fullDayLabel(f.from) })
      : t("logs.export.fromTo", "from {from} to {to}", { from: fullDayLabel(f.from), to: fullDayLabel(f.to) });
  return {
    "1h": t("logs.export.lastHour", "from the last hour"),
    "24h": t("logs.export.lastDay", "from the last 24 hours"),
    "7d": t("logs.export.lastWeek", "from the last 7 days"),
    "30d": t("logs.export.lastMonth", "from the last 30 days"),
  }[f.range ?? "24h"];
};

const list = (parts: string[]) => {
  const head = parts.slice(0, -1);
  const last = parts.slice(-1).join("");
  return head.length ? t("logs.export.list", "{first} and {last}", { first: head.join(t("logs.export.listSeparator", ", ")), last }) : last;
};

export function exportSummary(f: LogFilters, names: { key?: string | undefined; member?: string | undefined; model?: string | undefined } = {}): string {
  const kind = f.failed && f.stream === true ? "both" : f.failed ? "failed" : f.stream === true ? "streamed" : "none";
  const errorKind = (id: string) => (locale() === "en" ? errorKindLabel(id).toLowerCase() : errorKindLabel(id));
  const traits = [
    f.key ? t("logs.export.traitKey", "key {name}", { name: names.key ?? f.key }) : "",
    f.model ? t("logs.export.traitModel", "model {name}", { name: names.model ?? f.model }) : "",
    f.provider ? t("logs.export.traitProvider", "provider {name}", { name: providerLabel(f.provider) }) : "",
    f.status ? t("logs.export.traitStatus", "status {status}", { status: String(f.status) }) : "",
    f.error ? t("logs.export.traitError", "error kind {kind}", { kind: errorKind(f.error) }) : "",
  ].filter(Boolean);
  return t("logs.export.summary", "Every {kind}{source}request {period}{by}{traits}{notStreamed}, newest first.", {
    kind: t("logs.export.kind", "{kind, select, failed {failed } streamed {streamed } both {failed streamed } other {}}", { kind }),
    source: t("logs.export.source", "{source, select, playground {playground } api {API } other {}}", { source: f.source ?? "none" }),
    period: period(f),
    by: f.member ? t("logs.export.by", " by {name}", { name: names.member ?? f.member.replace(/^(user|sa):/, "") }) : "",
    traits: traits.length ? t("logs.export.with", " with {traits}", { traits: list(traits) }) : "",
    notStreamed: f.stream === false ? t("logs.export.notStreamed", ", not streamed") : "",
  });
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
