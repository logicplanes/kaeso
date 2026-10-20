import { Badge, InlineCopyText, Pill, ProviderBadge, StatusCode, Text, Tooltip, col } from "@logicplanes/kaeso-ui";
import { dateFormatter, formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import type { LogView, Party } from "~/api/generated/types";
import { errorKindLabel } from "~/format/labels";
import { requestUSD } from "~/format/money";
import { locale, t, type Locale } from "~/i18n";
import { common } from "~/i18n/common";
import { inputTokens, totalTokens } from "~/kaeso/tokens";
import { readStoredJson, writeStored } from "~/storage";
import { inZone } from "~/format/zone";

export const COLUMNS = [
  "time",
  "status",
  "endpoint",
  "model",
  "provider",
  "stream",
  "duration",
  "ttfb",
  "tokens",
  "cost",
  "key",
  "principal",
  "request",
  "agent",
] as const;
export type ColumnId = (typeof COLUMNS)[number];
export const columnLabel = (id: ColumnId): string =>
  ({
    time: t("logs.column.started", "Started"),
    status: common.status(),
    endpoint: t("logs.column.endpoint", "Endpoint"),
    model: common.model(),
    provider: common.provider(),
    stream: t("logs.column.stream", "Stream"),
    duration: t("logs.column.duration", "Duration"),
    ttfb: t("logs.column.firstByte", "First byte"),
    tokens: common.tokens(),
    cost: common.cost(),
    key: t("logs.column.key", "Key"),
    principal: t("logs.column.principal", "Principal"),
    request: t("logs.column.requestId", "Request id"),
    agent: t("logs.column.userAgent", "User agent"),
  })[id];
const DAY_TIME_WIDTH: Record<Locale, number> = { en: 148, de: 156, "zh-CN": 164 };
const PHONE_DAY_TIME_WIDTH: Record<Locale, number> = { en: 120, de: 136, "zh-CN": 128 };

function timeWidth(spansDays: boolean, phone: boolean): number {
  if (!spansDays) return phone ? 96 : 100;
  return (phone ? PHONE_DAY_TIME_WIDTH : DAY_TIME_WIDTH)[locale()];
}
const DEFAULT_COLUMNS: ColumnId[] = COLUMNS.filter((c) => c !== "request" && c !== "agent");

const COLUMNS_KEY = "kaeso:logs-columns";

export function storedColumns(): ColumnId[] {
  const v = readStoredJson(COLUMNS_KEY);
  return Array.isArray(v) ? (v.filter((c) => (COLUMNS as readonly string[]).includes(c)) as ColumnId[]) : DEFAULT_COLUMNS;
}

export function storeColumns(next: ColumnId[]): void {
  writeStored(COLUMNS_KEY, JSON.stringify(next));
}

export function logColumns({
  spansDays,
  phone = false,
  keyName,
  partyName,
  providerName,
}: {
  spansDays: boolean;
  phone?: boolean | undefined;
  keyName: (id: string) => string;
  partyName: (p: Party) => string;
  providerName: (id: string) => string;
}) {
  const time = (iso: string) =>
    dateFormatter(
      {
        ...(spansDays ? { day: "numeric", month: "short" } : {}),
        hour: "2-digit",
        minute: "2-digit",
        ...(phone && spansDays ? {} : { second: "2-digit" }),
        hourCycle: "h23",
        ...inZone(),
      },
      locale(),
    )(iso);
  return {
    time: col.custom(
      { id: "time", header: columnLabel("time"), size: timeWidth(spansDays, phone), priority: 1 },
      (r: LogView) => r.started_at,
      (v) => (
        <Text tabular>
          <time dateTime={v}>{time(v)}</time>
        </Text>
      ),
    ),
    status: col.custom(
      { id: "status", header: columnLabel("status"), size: 80, priority: 1 },
      (r: LogView) => r.status,
      (v) => <StatusCode code={v} />,
    ),
    endpoint: col.mono({ id: "endpoint", header: columnLabel("endpoint"), size: 152, priority: 3 }, (r: LogView) => r.endpoint),
    model: col.custom(
      { id: "model", header: columnLabel("model"), size: 240, minWidth: 104, priority: 1, grow: true },
      (r: LogView) => r.model,
      (_v, r) => <ModelCell row={r} />,
    ),
    provider: col.custom(
      { id: "provider", header: columnLabel("provider"), size: 144, priority: 3 },
      (r: LogView) => r.provider,
      (v) => <ProviderBadge provider={v} name={providerName(v)} withName size="sm" />,
    ),
    stream: col.custom(
      { id: "stream", header: columnLabel("stream"), size: 104, priority: 4 },
      (r: LogView) => r.stream,
      (v) =>
        v ? (
          <Pill size="sm" variant="outline">
            {t("logs.streamPill", "Stream")}
          </Pill>
        ) : null,
    ),
    duration: col.duration({ id: "duration", header: columnLabel("duration"), size: 104, priority: 2 }, (r: LogView) => r.duration_ms),
    ttfb: col.custom(
      { id: "ttfb", header: columnLabel("ttfb"), size: 104, priority: 4 },
      (r: LogView) => r.ttfb_ms ?? null,
      (v) => (v === null ? "—" : formatDuration(v)),
      { align: "end" },
    ),
    tokens: col.custom(
      { id: "tokens", header: t("logs.column.tokensInOut", "Tokens in / out"), size: 136, priority: 2 },
      (r: LogView) => totalTokens(r.usage),
      (_v, r) => <TokensCell row={r} />,
      { align: "end" },
    ),
    cost: col.custom(
      { id: "cost", header: columnLabel("cost"), size: 120, priority: 2 },
      (r: LogView) => r.cost,
      (v) => requestUSD(v),
      { align: "end" },
    ),
    key: col.custom(
      { id: "key", header: columnLabel("key"), size: 152, priority: 3 },
      (r: LogView) => r.key_id ?? null,
      (v) => (v ? keyName(v) : "—"),
    ),
    principal: col.custom(
      { id: "principal", header: columnLabel("principal"), size: 152, priority: 4 },
      (r: LogView) => r.principal_id,
      (_v, r) => partyName({ kind: r.principal_kind === "service_account" ? "service_account" : "user", id: r.principal_id }),
    ),
    request: col.custom(
      { id: "request", header: columnLabel("request"), size: 248, priority: 5 },
      (r: LogView) => r.id,
      (v) => <InlineCopyText value={v} label={t("logs.copyRequestId", "Copy request id")} />,
    ),
    agent: col.text({ id: "agent", header: columnLabel("agent"), size: 200, priority: 5 }, (r: LogView) => r.user_agent),
  };
}

const errorTone = (row: LogView) => (row.error_kind === "client_disconnected" ? "neutral" : row.status >= 500 ? "danger" : "warning");

function ModelCell({ row }: { row: LogView }) {
  const aliased = row.requested_model !== row.model && !row.requested_model.endsWith(`/${row.model}`);
  return (
    <span className="flex min-w-0 items-center gap-8">
      <Text mono variant="small" truncate>
        {row.model}
      </Text>
      {aliased && (
        <Text mono variant="small" tone="secondary" truncate className="hidden sm:block">
          {t("logs.fromModel", "from {model}", { model: row.requested_model })}
        </Text>
      )}
      {row.source === "playground" && (
        <Pill size="sm" variant="outline" className="hidden shrink-0 sm:inline-flex">
          {t("logs.source.playground", "Playground")}
        </Pill>
      )}
      {row.error_kind && (
        <Badge tone={errorTone(row)} className="hidden shrink-0 sm:inline-flex">
          {errorKindLabel(row.error_kind)}
        </Badge>
      )}
      {row.error_message && (
        <Text variant="small" tone="secondary" truncate className="hidden min-w-0 flex-1 sm:block">
          {row.error_message}
        </Text>
      )}
    </span>
  );
}

function TokensCell({ row }: { row: LogView }) {
  const u = row.usage;
  const detail = [
    u.cache_read_tokens && t("logs.tokens.cacheRead", "{count} cache read", { count: formatNumber(u.cache_read_tokens) }),
    u.cache_write_tokens && t("logs.tokens.cacheWrite", "{count} cache write", { count: formatNumber(u.cache_write_tokens) }),
    u.reasoning_tokens && t("logs.tokens.reasoning", "{count} reasoning", { count: formatNumber(u.reasoning_tokens) }),
  ].filter(Boolean);
  if (u.estimated) detail.unshift(t("logs.tokens.estimated", "Estimated: the stream ended before the provider's final count"));
  const text = `${u.estimated ? "≈ " : ""}${formatNumber(inputTokens(u))} / ${formatNumber(u.output_tokens)}`;
  return detail.length ? (
    <Tooltip content={detail.join(" · ")}>
      <Text tabular tabIndex={0}>
        {text}
      </Text>
    </Tooltip>
  ) : (
    <Text tabular>{text}</Text>
  );
}
