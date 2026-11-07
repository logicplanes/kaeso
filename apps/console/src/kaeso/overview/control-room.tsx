import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  Avatar,
  BarChart,
  Button,
  CellInset,
  Dashboard,
  DashboardCell,
  Legend,
  MetricTabs,
  ModelLabel,
  RankList,
  Readout,
  ShareBar,
  StatusCode,
  Text,
  TimeseriesChart,
  TOKEN_KIND_ORDER,
  tokenKindColor,
  type TokenKind,
  UsageBreakdown,
  type RankItem,
  type TimeSeries,
} from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatRelative, formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { bucketAxisLabel, bucketLabel, partialBuckets, spansYears } from "~/format/buckets";
import { errorKindLabel } from "~/format/labels";
import { dayFormat, dayLabel, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { periodUSD } from "~/kaeso/usage-panels";
import { projectPath } from "~/shell/pages";
import { Limit } from "./limit";
import { utcTimeFormat } from "./timeline";
import type { Overview } from "./data";
import { cacheHit, change, METRICS, metricTotal, type Dimension, type MetricId } from "./metrics";

const monthName = dayFormat({ month: "long" });
const minuteLabel = utcTimeFormat({ hour: "2-digit", minute: "2-digit" });
const hourLabel = utcTimeFormat({ weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const PREVIOUS = "var(--k-text-tertiary)";

const KIND_LABEL: Record<TokenKind, () => string> = {
  input: () => t("kaeso.overview.kind.input", "Input"),
  cacheRead: () => t("kaeso.overview.kind.cacheRead", "Cache read"),
  cacheWrite: () => t("kaeso.overview.kind.cacheWrite", "Cache write"),
  output: () => t("kaeso.overview.kind.output", "Output"),
  reasoning: () => t("kaeso.overview.kind.reasoning", "Reasoning"),
};

const KIND_OF: Record<TokenKind, (r: UsageRow) => number> = {
  input: (r) => r.input_tokens,
  cacheRead: (r) => r.cache_read_tokens,
  cacheWrite: (r) => r.cache_write_tokens,
  output: (r) => r.output_tokens - r.reasoning_tokens,
  reasoning: (r) => r.reasoning_tokens,
};

function series(o: Overview, id: MetricId, withPrevious: boolean, detailed = false): TimeSeries[] {
  const m = METRICS[id];
  if (detailed && id === "tokens")
    return TOKEN_KIND_ORDER.map((k) => ({
      id: `tokens-${k}`,
      label: KIND_LABEL[k](),
      color: tokenKindColor(k),
      points: o.points.map((x) => ({ t: x.t, v: x.row ? Math.max(0, KIND_OF[k](x.row)) : 0 })),
    }));
  const now: TimeSeries = {
    id,
    label: o.compare ? t("kaeso.overview.thisPeriod", "This period") : m.label(),
    color: m.color,
    points: o.points.map((x) => ({ t: x.t, v: m.of(x.row) })),
  };
  const failed: TimeSeries[] =
    detailed && id === "requests"
      ? [
          {
            id: "requests-failed",
            label: t("kaeso.overview.failed", "Failed requests"),
            color: METRICS.errors.color,
            points: o.points.map((x) => ({ t: x.t, v: x.row?.errors ?? 0 })),
          },
        ]
      : [];
  if (!withPrevious || !o.prevRows) return [now, ...failed];
  const before: TimeSeries = {
    id: `${id}-before`,
    label: o.compareLabel,
    color: PREVIOUS,
    dashed: true,
    points: o.points.map((x, i) => ({ t: x.t, v: m.of(o.prevPoints[i]?.row) })),
  };
  return [now, ...failed, before];
}

function MetricChart({
  o,
  id,
  height,
  previous = true,
  detailed = false,
}: {
  o: Overview;
  id: MetricId;
  height: number;
  previous?: boolean;
  detailed?: boolean;
}) {
  const m = METRICS[id];
  const stacked = detailed && id === "tokens";
  const withYear = spansYears(o.from, o.to, o.g);
  return (
    <TimeseriesChart
      aria-label={m.label()}
      series={series(o, id, previous, detailed)}
      kind={stacked ? "stacked" : "area"}
      height={height}
      hideLegend
      shares={false}
      formatValue={m.format}
      loading={o.pointsLoading}
      error={o.daily.isError && t("kaeso.usage.chartFailed", "Couldn't load this chart")}
      onRetry={() => void o.daily.refetch()}
      empty={!o.daily.isPending && o.rows.length === 0}
      emptyText={t("kaeso.usage.noRequests", "No requests in this period.")}
      timeZone="UTC"
      {...(o.step
        ? { tickAt: "time" as const, formatTime: o.step === "minute" ? minuteLabel : hourLabel, partial: { ongoing: o.live } }
        : {
            tickAt: "points" as const,
            formatTime: (time: Date) => bucketLabel(utcDay(time), o.g, withYear),
            formatTick: (time: Date) => bucketAxisLabel(utcDay(time), o.g),
            partial: partialBuckets(o.from, o.to, o.g, utcDay(new Date())),
          })}
    />
  );
}

const changeOf = (o: Overview, id: MetricId, better = METRICS[id].better) => {
  if (!o.prevRows) return undefined;
  const c = change(metricTotal(id, o.rows), metricTotal(id, o.prevRows));
  return c === undefined ? undefined : { value: c, better };
};

/** Tall enough that the chart's cell ends where the column of totals beside it does. */
const chartHeight = (o: Overview) => Math.max(260, Math.min(480, (3 + Number(o.showCost) + Number(o.now.toolCalls > 0)) * 134 - (o.prevRows ? 140 : 116)));

const perBucket = (o: Overview, metric: string) =>
  t("kaeso.overview.perBucket", "{metric} per {granularity, select, minute {minute} hour {hour} week {week} month {month} other {day}}, UTC", {
    metric,
    granularity: o.step ?? o.g,
  });

function DayContext({ o, context, metric, height }: { o: Overview; context: NonNullable<Overview["context"]>; metric: MetricId; height: number }) {
  const m = METRICS[metric];
  const days = context.days;
  const last = days.length - 1;
  return (
    <BarChart
      aria-label={m.label()}
      categories={days.map((d) => dayLabel(d.day))}
      dates={days.map((d) => d.day)}
      series={[
        {
          id: "earlier",
          label: t("kaeso.overview.earlierDays", "Earlier days"),
          color: "var(--k-fill-3)",
          values: days.map((d, i) => (i === last ? 0 : m.of(d.row))),
        },
        { id: "today", label: dayLabel(o.to), color: m.color, values: days.map((d, i) => (i === last ? m.of(d.row) : 0)) },
      ]}
      layout="stacked"
      shares={false}
      height={height}
      formatValue={m.format}
      loading={context.query.isPending}
      empty={!context.query.isPending && days.every((d) => !d.row)}
      emptyText={t("kaeso.usage.noRequests", "No requests in this period.")}
    />
  );
}

function legendFor(o: Overview, metric: MetricId) {
  const m = METRICS[metric];
  const sum = (f: (r: UsageRow) => number) => o.rows.reduce((a, r) => a + f(r), 0);
  if (metric === "tokens")
    return TOKEN_KIND_ORDER.map((k) => ({
      id: k,
      label: KIND_LABEL[k](),
      value: formatTokens(sum((r) => Math.max(0, KIND_OF[k](r)))),
      color: tokenKindColor(k),
    }));
  return [
    { id: "now", label: o.compare ? t("kaeso.overview.thisPeriod", "This period") : m.label(), value: m.format(metricTotal(metric, o.rows)), color: m.color },
    ...(metric === "requests"
      ? [{ id: "failed", label: t("kaeso.overview.failed", "Failed requests"), value: formatNumber(o.now.errors), color: METRICS.errors.color }]
      : []),
    ...(o.prevRows ? [{ id: "before", label: o.compareLabel, value: m.format(metricTotal(metric, o.prevRows)), color: PREVIOUS, dashed: true }] : []),
  ];
}

function ChartCell({ o, metric, className }: { o: Overview; metric: MetricId; className: string }) {
  const m = METRICS[metric];
  const what = metric === "errors" ? t("kaeso.overview.failedShare", "Failed requests") : m.label();
  return (
    <DashboardCell
      className={className}
      title={o.context && !o.step ? t("kaeso.overview.lastDays", "{metric} per day, the last 14 days, UTC", { metric: what }) : perBucket(o, what)}
    >
      <CellInset grow>
        <Legend items={legendFor(o, metric)} />
        <div id="overview-chart" className={o.prevRows ? "mt-16" : ""}>
          {o.context && !o.step ? (
            <DayContext o={o} context={o.context} metric={metric} height={chartHeight(o)} />
          ) : (
            <MetricChart o={o} id={metric} height={chartHeight(o)} detailed />
          )}
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function SpendCell({ o, className }: { o: Overview; className: string }) {
  const split = (o.project ? o.grouped.model : o.grouped.project)?.data ?? [];
  const priced = [...split].filter((r) => Number(r.cost ?? 0) > 0).sort((a, b) => Number(b.cost) - Number(a.cost));
  const top = priced.slice(0, 3).map((r) => ({
    id: r.key,
    label: o.project ? o.modelName(r.key) : (o.projectOf(r.key)?.name ?? r.key),
    value: Number(r.cost),
  }));
  const rest = priced.slice(3).reduce((a, r) => a + Number(r.cost), 0);
  const shown = rest > 0 ? [...top, { id: "other", label: t("kaeso.overview.other", "Other"), value: rest, color: "var(--k-fill-3)" }] : top;
  const month = o.forecast ? monthName(o.forecast.month) : "";
  const splitLabel = o.project ? t("kaeso.overview.costByModel", "Cost by model") : t("kaeso.overview.costByProject", "Cost by project");
  return (
    <DashboardCell
      className={className}
      title={o.forecast ? t("kaeso.overview.forecast", "Forecast for {month}", { month }) : t("kaeso.overview.spend", "Spend")}
      actions={
        o.project &&
        o.can("limits.view") && (
          <Button size="sm" variant="ghost" href={`${projectPath(o.org, o.project)}/limits`}>
            {t("kaeso.overview.limits", "Limits")}
          </Button>
        )
      }
    >
      {o.forecast ? (
        <Readout
          size="sm"
          value={formatUSD(o.forecast.projected.toFixed(2))}
          detail={t("kaeso.overview.soFar", "{amount} so far, at the last 7 days' pace", { amount: formatUSD(o.forecast.soFar.toFixed(2)) })}
          loading={o.loading}
        />
      ) : (
        <Readout
          size="sm"
          value={periodUSD(o.now.cost ?? "0")}
          change={changeOf(o, "cost")}
          detail={changeOf(o, "cost") ? o.compare : undefined}
          loading={o.loading}
        />
      )}
      {o.spend && (
        <CellInset>
          <div className="mt-20">
            <Limit limit={o.spend} />
          </div>
        </CellInset>
      )}
      {shown.length > 0 && (
        <CellInset>
          <div className="mt-24 flex flex-col gap-12">
            <Text variant="small" tone="tertiary">
              {splitLabel}
            </Text>
            <ShareBar aria-label={splitLabel} slices={shown} formatValue={(v) => periodUSD(v.toFixed(6))} />
          </div>
        </CellInset>
      )}
    </DashboardCell>
  );
}

function TokenCell({ o, className }: { o: Overview; className: string }) {
  const sum = (f: (r: UsageRow) => number) => o.rows.reduce((a, r) => a + f(r), 0);
  const n = o.now.requests || 1;
  const input = sum((r) => r.input_tokens + r.cache_read_tokens + r.cache_write_tokens);
  return (
    <DashboardCell className={className} title={t("kaeso.overview.tokenMix", "Tokens per request")}>
      <Readout
        size="sm"
        value={formatTokens(Math.round(o.now.tokens / n))}
        detail={t("kaeso.overview.inOut", "{input} in, {output} out", {
          input: formatTokens(Math.round(input / n)),
          output: formatTokens(Math.round(sum((r) => r.output_tokens) / n)),
        })}
        loading={o.loading}
      />
      <CellInset>
        <div className="mt-20">
          <UsageBreakdown
            tokens={{
              input: sum((r) => r.input_tokens),
              cacheRead: sum((r) => r.cache_read_tokens),
              cacheWrite: sum((r) => r.cache_write_tokens),
              output: sum((r) => r.output_tokens - r.reasoning_tokens),
              reasoning: sum((r) => r.reasoning_tokens),
            }}
          />
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function CacheCell({ o, className }: { o: Overview; className: string }) {
  const hit = cacheHit(o.rows);
  const read = o.rows.reduce((a, r) => a + r.cache_read_tokens, 0);
  return (
    <DashboardCell className={className} title={t("kaeso.overview.cacheHit", "Cache hit")}>
      <Readout
        size="sm"
        value={hit === null ? "—" : formatPercent(hit, 0)}
        change={changeOf(o, "cache")}
        detail={t("kaeso.overview.cacheRead", "{tokens} input tokens from cache", { tokens: formatTokens(read) })}
        loading={o.loading}
      />
      <CellInset>
        <div className="flex flex-col">
          <div className="mt-16">
            {o.context && !o.step ? (
              <DayContext o={o} context={o.context} metric="cache" height={160} />
            ) : (
              <MetricChart o={o} id="cache" height={160} previous={false} />
            )}
          </div>
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function ReliabilityCell({ o, className }: { o: Overview; className: string }) {
  const navigate = useNavigate();
  const base = o.project ? projectPath(o.org, o.project) : "";
  const recent: RankItem[] = (o.errors.data ?? []).map((l) => ({
    id: l.id,
    label: (
      <>
        <StatusCode code={l.status} />
        <span className="min-w-0 truncate">{l.error_kind ? errorKindLabel(l.error_kind) : l.requested_model}</span>
      </>
    ),
    secondary: formatRelative(l.started_at),
  }));
  const byGroup = rank(o, o.project ? "model" : "project", "errors", 4, true);
  const showRecent = o.project && o.live && o.showErrors;
  return (
    <DashboardCell
      className={className}
      title={
        showRecent
          ? t("kaeso.overview.failed", "Failed requests")
          : o.project
            ? t("kaeso.overview.failedByModel", "Failed requests by model")
            : t("kaeso.overview.failedByProject", "Failed requests by project")
      }
      actions={
        o.showErrors && (
          <Button size="sm" variant="ghost" href={`${base}/logs`}>
            {t("kaeso.overview.requestLogs", "Request logs")}
          </Button>
        )
      }
    >
      <Readout
        size="sm"
        value={formatNumber(o.now.errors)}
        change={changeOf(o, "errors")}
        detail={t("kaeso.overview.ofRequests", "{rate} of requests", { rate: formatPercent(o.now.requests ? o.now.errors / o.now.requests : 0) })}
        loading={o.loading}
      />
      <div className="mt-12">
        {showRecent ? (
          <RankList
            aria-label={t("project.overview.recentErrors", "Recent errors")}
            items={recent}
            loading={o.errors.isPending}
            rows={4}
            empty={t("project.overview.noErrors", "No failed requests.")}
            onOpen={(id) => void navigate({ href: `${base}/logs?log=${encodeURIComponent(id)}` })}
          />
        ) : (
          <RankList
            aria-label={
              o.project ? t("kaeso.overview.failedByModel", "Failed requests by model") : t("kaeso.overview.failedByProject", "Failed requests by project")
            }
            items={byGroup.items}
            loading={byGroup.loading}
            rows={4}
            empty={t("project.overview.noErrors", "No failed requests.")}
            {...(o.project ? {} : { onOpen: (id: string) => void navigate({ href: projectPath(o.org, id) }) })}
          />
        )}
      </div>
    </DashboardCell>
  );
}

type Ranked = Exclude<Dimension, "member">;

function rank(o: Overview, dim: Ranked, metric: MetricId, limit: number, compact = false): { items: RankItem[]; loading: boolean } {
  const q = o.grouped[dim];
  const m = METRICS[metric];
  const rows = [...(q?.data ?? [])].filter((r) => m.rank(r) > 0).sort((a, b) => m.rank(b) - m.rank(a));
  const total = rows.reduce((a, r) => a + m.rank(r), 0) || 1;
  const label = (r: UsageRow) => {
    if (dim === "model") {
      const [provider = "", ...rest] = r.key.split("/");
      const model = rest.join("/");
      if (!model) return <Text tone="secondary">{o.modelName(r.key)}</Text>;
      const name = o.modelName(r.key);
      return (
        <ModelLabel className="max-w-full" provider={provider} providerName={o.providers.name(provider)} model={model} {...(name !== model ? { name } : {})} />
      );
    }
    if (dim === "project") {
      const p = o.projectOf(r.key);
      return (
        <>
          <Avatar name={p?.name ?? r.key} {...(p?.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
          <span className="min-w-0 truncate">{p?.name ?? r.key}</span>
        </>
      );
    }
    return <span className="min-w-0 truncate">{o.keyName(r.key)}</span>;
  };
  return {
    loading: !q || q.isPending || o.namesLoading,
    items: rows.slice(0, limit).map((r) => ({
      id: r.key,
      label: label(r),
      ...(compact
        ? { value: formatPercent(r.errors / r.requests) }
        : {
            value: m.formatRank(m.rank(r)),
            secondary: metric === "errors" ? formatPercent(r.errors / r.requests) : formatPercent(m.rank(r) / total, 0),
          }),
      share: m.rank(r) / total,
    })),
  };
}

const DIM_TITLE: Record<Ranked, (by: string) => string> = {
  project: (by) => t("kaeso.overview.topProjects", "Projects by {metric}", { metric: by }),
  model: (by) => t("kaeso.overview.topModels", "Models by {metric}", { metric: by }),
  key: (by) => t("kaeso.overview.topKeys", "Keys by {metric}", { metric: by }),
};

function TopCell({ o, dim, metric, className }: { o: Overview; dim: Ranked; metric: MetricId; className: string }) {
  const navigate = useNavigate();
  const r = rank(o, dim, metric, 5);
  const title = DIM_TITLE[dim](metric === "errors" ? t("kaeso.overview.failures", "failed requests") : METRICS[metric].label().toLowerCase());
  const logs = o.project && o.showErrors ? `${projectPath(o.org, o.project)}/logs` : undefined;
  const filter = (id: string) => (dim === "model" ? `model=${encodeURIComponent(id.split("/").slice(1).join("/"))}` : `${dim}=${encodeURIComponent(id)}`);
  const open =
    dim === "project"
      ? (id: string) => void navigate({ href: projectPath(o.org, id) })
      : logs
        ? (id: string) => void navigate({ href: `${logs}?${filter(id)}` })
        : undefined;
  return (
    <DashboardCell className={className} title={title}>
      <RankList
        aria-label={title}
        items={r.items}
        loading={r.loading}
        empty={t("kaeso.overview.nothing", "Nothing in this period")}
        {...(open ? { onOpen: open } : {})}
      />
    </DashboardCell>
  );
}

export function ControlRoom({ o, initial = "requests" }: { o: Overview; initial?: MetricId | undefined }) {
  const [metric, setMetric] = useState<MetricId>(initial);
  const ids: MetricId[] = ["requests", "tokens", ...(o.showCost ? (["cost"] as const) : []), "errors", ...(o.now.toolCalls > 0 ? (["tools"] as const) : [])];
  const top: Ranked[] = o.project ? ["model", "key"] : ["project", "model"];
  return (
    <Dashboard aria-label={t("kaeso.overview.usage", "Usage in the period")} busy={o.refreshing} columns={2} wideColumns={4}>
      <ChartCell o={o} metric={metric} className="lg:col-span-2 xl:col-span-3" />
      <MetricTabs
        aria-label={t("kaeso.overview.metric", "Metric to chart")}
        orientation="vertical"
        className="lg:col-span-2 xl:col-span-1"
        controls="overview-chart"
        loading={o.loading}
        value={metric}
        onValueChange={(v) => setMetric(v as MetricId)}
        items={ids.map((id) => {
          const c = changeOf(o, id);
          const m = METRICS[id];
          if (o.daily.isError) return { id, label: m.label(), value: "—", detail: t("kaeso.usage.tileFailed", "Couldn't load") };
          return {
            id,
            label: m.label(),
            value: m.format(metricTotal(id, o.rows)),
            ...(c ? { change: c } : {}),
            detail:
              id === "errors"
                ? t("kaeso.overview.failedCount", "{count} failed", { count: formatNumber(o.now.errors) })
                : c
                  ? o.compare
                  : o.single && o.to === utcDay(new Date())
                    ? t("kaeso.overview.soFarToday", "So far today")
                    : undefined,
            trend: (o.context && !o.step ? o.context.days : o.points).map((d) => m.of(d.row)),
            trendColor: m.color,
            trendLabel: perBucket(o, m.label()),
          };
        })}
      />
      {/* Two columns from 1024px and four from 1280px: every cell edge continues a column edge at both widths. */}
      <CacheCell o={o} className={o.showCost ? "lg:col-span-1 xl:col-span-1" : "lg:col-span-1 xl:col-span-2"} />
      <TokenCell o={o} className="lg:col-span-1 xl:col-span-1" />
      {o.showCost && <SpendCell o={o} className="lg:col-span-1 xl:col-span-1" />}
      <ReliabilityCell o={o} className={o.showCost ? "lg:col-span-1 xl:col-span-1" : "lg:col-span-2 xl:col-span-1"} />
      {top.map((d) => (
        <TopCell key={d} o={o} dim={d} metric={metric} className="lg:col-span-1 xl:col-span-2" />
      ))}
    </Dashboard>
  );
}
