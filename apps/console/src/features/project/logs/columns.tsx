import type { ReactNode } from "react";
import { Badge, InlineCopyText, Pill, ProviderBadge, StatusCode, Text, Tooltip, col } from "@logicplanes/kaeso-ui";
import { dateFormatter, formatDuration, formatNumber, formatPercent } from "@logicplanes/kaeso-ui/format";
import type { LogView, Party } from "~/api/generated/types";
import { errorKindLabel } from "~/format/labels";
import { requestUSD } from "~/format/money";
import { locale, t, type Locale } from "~/i18n";
import { common } from "~/i18n/common";
import { inputTokens, totalTokens } from "~/kaeso/tokens";
import { readStoredJson, writeStored } from "~/storage";
import { inZone } from "~/format/zone";
import { CodeText } from "~/kaeso/code-text";
import { logMessage, type LogMessage, type useGatewayErrors } from "./messages";

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
const SHORT_DAY_TIME_WIDTH: Record<Locale, number> = { en: 128, de: 136, "zh-CN": 144 };
const TIME_WIDTH: Record<Locale, number> = { en: 128, de: 128, "zh-CN": 144 };
const STACKED_WIDTH = 88;
const PHONE_TIME_WIDTH = 96;
const PROVIDER_WIDTH = 144;
const MODEL_WIDTH = 280;
const NARROW_MODEL_WIDTH = 160;
const NARROW_BELOW = 800;

export type LogsLayout = "wide" | "medium" | "narrow" | "phone";

function timeWidth(spansDays: boolean, layout: LogsLayout): number {
  if (layout === "phone" && spansDays) return STACKED_WIDTH;
  if (layout === "phone") return PHONE_TIME_WIDTH;
  if (!spansDays) return TIME_WIDTH[locale()];
  return (layout === "narrow" ? SHORT_DAY_TIME_WIDTH : DAY_TIME_WIDTH)[locale()];
}

const ROW_END_RESERVE = 48;
const STATUS_WIDTH = 80;
const KEPT_WIDTHS = { duration: 104, tokens: 184, cost: 120 } as const;

export function logsLayout(width: number, phone: boolean, spansDays: boolean, shown: readonly ColumnId[]): LogsLayout {
  if (phone) return "phone";
  if (!width) return "wide";
  if (width < NARROW_BELOW) return "narrow";
  const kept = (Object.keys(KEPT_WIDTHS) as (keyof typeof KEPT_WIDTHS)[]).filter((c) => shown.includes(c));
  const full =
    ROW_END_RESERVE +
    (shown.includes("time") ? timeWidth(spansDays, "wide") : 0) +
    (shown.includes("status") ? STATUS_WIDTH : 0) +
    (shown.includes("model") ? MODEL_WIDTH : 0) +
    (shown.includes("provider") ? PROVIDER_WIDTH : 0) +
    kept.reduce((sum, c) => sum + KEPT_WIDTHS[c], 0);
  return width < full ? "medium" : "wide";
}

function zoneName(): string {
  const parts = new Intl.DateTimeFormat(locale() === "en" ? "en-GB" : locale(), { timeZoneName: "short", ...inZone() }).formatToParts(new Date());
  return parts.find((p) => p.type === "timeZoneName")?.value ?? "UTC";
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

export const providerInModel = (layout: LogsLayout, shown: readonly ColumnId[]) =>
  shown.includes("provider") && shown.includes("model") && (layout === "medium" || layout === "narrow");

export function logColumns({
  spansDays,
  layout = "wide",
  withProvider = false,
  keyName,
  partyName,
  providerName,
  errors,
}: {
  spansDays: boolean;
  layout?: LogsLayout | undefined;
  withProvider?: boolean | undefined;
  errors?: ReturnType<typeof useGatewayErrors>;
  keyName: (id: string) => string;
  partyName: (p: Party) => string;
  providerName: (id: string) => string;
}) {
  const stacked = layout === "phone" && spansDays;
  const format = (iso: string, parts: Intl.DateTimeFormatOptions) => dateFormatter({ ...parts, hourCycle: "h23", ...inZone() }, locale())(iso);
  const time = (iso: string) =>
    format(iso, {
      ...(spansDays ? { day: "numeric", month: "short" } : {}),
      hour: "2-digit",
      minute: "2-digit",
      ...(layout === "narrow" && spansDays ? {} : { second: "2-digit" }),
    });
  return {
    time: col.custom(
      {
        id: "time",
        header: layout === "phone" ? columnLabel("time") : t("logs.column.startedIn", "Started ({zone})", { zone: zoneName() }),
        size: timeWidth(spansDays, layout),
        priority: 1,
      },
      (r: LogView) => r.started_at,
      (v) =>
        stacked ? (
          <time dateTime={v} className="flex flex-col">
            <Text tabular>{format(v, { hour: "2-digit", minute: "2-digit" })}</Text>
            <Text variant="caption" tone="tertiary">
              {format(v, { day: "numeric", month: "short" })}
            </Text>
          </time>
        ) : (
          <Text tabular>
            <time dateTime={v}>{time(v)}</time>
          </Text>
        ),
    ),
    status: col.custom(
      { id: "status", header: columnLabel("status"), size: STATUS_WIDTH, priority: 1 },
      (r: LogView) => r.status,
      (v) => <StatusCode code={v} />,
    ),
    endpoint: col.mono({ id: "endpoint", header: columnLabel("endpoint"), size: 152, priority: 3 }, (r: LogView) => r.endpoint),
    model: col.custom(
      {
        id: "model",
        header: columnLabel("model"),
        size: layout === "narrow" || layout === "phone" ? NARROW_MODEL_WIDTH : MODEL_WIDTH,
        minWidth: 104,
        priority: 1,
        grow: true,
      },
      (r: LogView) => r.model,
      (_v, r) => (
        <ModelCell
          row={r}
          message={logMessage(errors, r.error_message, r.provider, providerName)}
          provider={withProvider && r.provider ? <ProviderMark provider={r.provider} name={providerName(r.provider)} /> : null}
        />
      ),
    ),
    provider: col.custom(
      { id: "provider", header: columnLabel("provider"), size: PROVIDER_WIDTH, priority: layout === "phone" ? 2 : 1 },
      (r: LogView) => r.provider,
      (v) => (v ? <ProviderBadge provider={v} name={providerName(v)} withName size="sm" /> : null),
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
    duration: col.duration({ id: "duration", header: columnLabel("duration"), size: KEPT_WIDTHS.duration, priority: 2 }, (r: LogView) => r.duration_ms),
    ttfb: col.custom(
      { id: "ttfb", header: columnLabel("ttfb"), size: 104, priority: 4 },
      (r: LogView) => r.ttfb_ms ?? null,
      (v) => (v === null ? "—" : formatDuration(v)),
      { align: "end" },
    ),
    tokens: col.custom(
      { id: "tokens", header: t("logs.column.tokensInOutCached", "Tokens in / out · cached"), size: KEPT_WIDTHS.tokens, priority: 2 },
      (r: LogView) => totalTokens(r.usage),
      (_v, r) => <TokensCell row={r} />,
      { align: "end" },
    ),
    cost: col.custom(
      { id: "cost", header: columnLabel("cost"), size: KEPT_WIDTHS.cost, priority: 2 },
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

function ProviderMark({ provider, name }: { provider: string; name: string }) {
  return (
    <Tooltip content={name}>
      <Text tabIndex={0} className="flex shrink-0">
        <ProviderBadge provider={provider} name={name} size="sm" />
      </Text>
    </Tooltip>
  );
}

function ModelCell({ row, message, provider }: { row: LogView; message: LogMessage | null; provider: ReactNode }) {
  const aliased = !!row.model && row.requested_model !== row.model && !row.requested_model.endsWith(`/${row.model}`);
  return (
    <span className="@container flex w-full min-w-0 items-center gap-8">
      {provider}
      {row.model && (
        <Text mono variant="small" truncate>
          {row.model}
        </Text>
      )}
      {row.error_kind && (
        <Badge tone={errorTone(row)} className="hidden shrink-0 @cell-sm:inline-flex">
          {errorKindLabel(row.error_kind)}
        </Badge>
      )}
      {row.source === "playground" && (
        <Pill size="sm" variant="outline" className={row.error_kind ? "hidden shrink-0 @cell-md:inline-flex" : "hidden shrink-0 @cell-sm:inline-flex"}>
          {t("logs.source.playground", "Playground")}
        </Pill>
      )}
      {aliased && (
        <Text mono variant="small" tone="secondary" truncate className="hidden max-w-240 @cell-lg:block">
          {t("logs.fromModel", "from {model}", { model: row.requested_model })}
        </Text>
      )}
      {message && (
        <Text variant="small" tone="secondary" truncate className="hidden min-w-0 flex-1 basis-0 @cell-lg:block">
          <CodeText
            text={message.provider ? t("logs.providerSaid", "{provider}: {message}", { provider: message.provider, message: message.text }) : message.text}
          />
        </Text>
      )}
    </span>
  );
}

export const isEmbedding = (row: Pick<LogView, "ingress_dialect" | "egress_dialect">) =>
  row.ingress_dialect === "openai_embeddings" || row.egress_dialect === "openai_embeddings";

export const cachedShare = (u: LogView["usage"]): number | null => {
  const input = inputTokens(u);
  return input > 0 ? u.cache_read_tokens / input : null;
};

export const sentWithoutBreakpoint = (row: LogView) =>
  row.cache_breakpoints === 0 && inputTokens(row.usage) > 0 && row.usage.cache_read_tokens === 0 && row.usage.cache_write_tokens === 0;

function TokensCell({ row }: { row: LogView }) {
  const u = row.usage;
  const share = isEmbedding(row) ? null : cachedShare(u);
  const unmarked = sentWithoutBreakpoint(row);
  const detail = [
    unmarked && t("logs.tokens.noBreakpoint", "Sent without a cache breakpoint, and nothing was cached"),
    u.cache_read_tokens && t("logs.tokens.cacheRead", "{count} cache read", { count: formatNumber(u.cache_read_tokens) }),
    u.cache_write_tokens && t("logs.tokens.cacheWrite", "{count} cache write", { count: formatNumber(u.cache_write_tokens) }),
    u.reasoning_tokens && t("logs.tokens.reasoning", "{count} reasoning", { count: formatNumber(u.reasoning_tokens) }),
  ].filter(Boolean);
  if (u.estimated) detail.unshift(t("logs.tokens.estimated", "Estimated: the stream ended before the provider's final count"));
  const text = `${u.estimated ? "≈ " : ""}${formatNumber(inputTokens(u))} / ${formatNumber(u.output_tokens)}`;
  const content = (
    <span className="inline-flex items-baseline gap-6">
      <Text tabular>{text}</Text>
      {share !== null && (
        <Text variant="small" tone={unmarked ? "warning" : "tertiary"} tabular>
          {`· ${formatPercent(share, 0)}`}
        </Text>
      )}
    </span>
  );
  return detail.length ? (
    <Tooltip content={detail.join(" · ")}>
      <Text tabIndex={0}>{content}</Text>
    </Tooltip>
  ) : (
    content
  );
}
