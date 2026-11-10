import { useNavigate } from "@tanstack/react-router";
import { Avatar, BarChart, ModelLabel, Text, TimeseriesChart, type RankItem, type TimeSeries } from "@logicplanes/kaeso-ui";
import { formatPercent } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { bucketAxisLabel, bucketLabel, partialBuckets, spansYears } from "~/format/buckets";
import { dayLabel, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { projectPath } from "~/shell/pages";
import type { Overview } from "./data";
import { change, METRICS, type Dimension, type MetricId } from "./metrics";
import { utcTimeFormat } from "./timeline";

export const hourLabel = utcTimeFormat({ weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
export const PREVIOUS = "var(--k-text-tertiary)";

/** The period as the request logs and the breakdown tabs read it from their address. */
export const logsRange = (o: Overview) => `from=${o.from}&to=${o.to}`;
export const tabRange = (o: Overview) => (o.period ? `period=${o.period}` : logsRange(o));

function series(o: Overview, id: MetricId): TimeSeries[] {
  const m = METRICS[id];
  const now: TimeSeries = {
    id,
    label: o.compare ? t("kaeso.overview.thisPeriod", "This period") : m.label(),
    color: m.color,
    points: o.points.map((x) => ({ t: x.t, v: m.of(x.row) })),
  };
  if (!o.prevRows) return [now];
  return [
    now,
    { id: `${id}-before`, label: o.compareLabel, color: PREVIOUS, dashed: true, points: o.points.map((x, i) => ({ t: x.t, v: m.of(o.prevPoints[i]?.row) })) },
  ];
}

/** The period's chart of one metric: by the hour for a day or two, by the day, week or month beyond. */
export function MetricChart({ o, id, height, previous = true }: { o: Overview; id: MetricId; height: number; previous?: boolean }) {
  const m = METRICS[id];
  const withYear = spansYears(o.from, o.to, o.g);
  return (
    <TimeseriesChart
      aria-label={m.label()}
      series={previous ? series(o, id) : series(o, id).slice(0, 1)}
      kind="area"
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
        ? { tickAt: "time" as const, formatTime: hourLabel, partial: { ongoing: o.live } }
        : {
            tickAt: "points" as const,
            formatTime: (time: Date) => bucketLabel(utcDay(time), o.g, withYear),
            formatTick: (time: Date) => bucketAxisLabel(utcDay(time), o.g),
            partial: partialBuckets(o.from, o.to, o.g, utcDay(new Date())),
          })}
    />
  );
}

/** A single day without the request log has one point, so it stands among the fourteen days up to it. */
export function DayContext({ o, context, metric, height }: { o: Overview; context: NonNullable<Overview["context"]>; metric: MetricId; height: number }) {
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

export type Ranked = Exclude<Dimension, "member">;

function entityLabel(o: Overview, dim: Ranked, key: string) {
  if (dim === "model") {
    const [provider = "", ...rest] = key.split("/");
    const model = rest.join("/");
    if (!model) return <Text tone="secondary">{o.modelName(key)}</Text>;
    const name = o.modelName(key);
    return (
      <ModelLabel className="max-w-full" provider={provider} providerName={o.providers.name(provider)} model={model} {...(name !== model ? { name } : {})} />
    );
  }
  if (dim === "project") {
    const p = o.projectOf(key);
    return (
      <>
        <Avatar name={p?.name ?? key} {...(p?.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
        <span className="min-w-0 truncate">{p?.name ?? key}</span>
      </>
    );
  }
  return <span className="min-w-0 truncate">{o.keyName(key)}</span>;
}

/** The rows of one grouping, largest first by the metric, each with its share and its change against the period before. */
export function rank(o: Overview, dim: Ranked, metric: MetricId, limit: number): { items: RankItem[]; loading: boolean; count: number } {
  const q = o.grouped[dim];
  const before = o.groupedBefore[dim];
  const m = METRICS[metric];
  const rows = [...(q?.data ?? [])].filter((r) => m.rank(r) > 0).sort((a, b) => m.rank(b) - m.rank(a));
  const total = rows.reduce((a, r) => a + m.rank(r), 0) || 1;
  const was = before?.data ? new Map(before.data.map((r) => [r.key, m.rank(r)])) : undefined;
  const moved = (r: UsageRow) => {
    if (!was) return {};
    const c = change(m.rank(r), was.get(r.key));
    return { change: c === undefined ? t("kaeso.analytics.newInPeriod", "New") : { value: c, better: m.better } };
  };
  return {
    loading: !q || q.isPending || o.namesLoading,
    count: rows.length,
    items: rows.slice(0, limit).map((r) => ({
      id: r.key,
      label: entityLabel(o, dim, r.key),
      value: m.formatRank(m.rank(r)),
      secondary: metric === "errors" ? formatPercent(r.errors / r.requests) : formatPercent(m.rank(r) / total, 0),
      ...moved(r),
      share: m.rank(r) / total,
    })),
  };
}

/** Where a row of a grouping leads: the project itself, or the request logs of the period with the row's filter. */
export function useOpenRow(o: Overview, dim: Ranked, failed: boolean): ((id: string) => void) | undefined {
  const navigate = useNavigate();
  if (dim === "project") return (id) => void navigate({ href: projectPath(o.org, id) });
  if (!o.project || !o.showErrors) return undefined;
  const logs = `${projectPath(o.org, o.project)}/logs`;
  const filter = (id: string) => (dim === "model" ? `model=${encodeURIComponent(id.split("/").slice(1).join("/"))}` : `key=${encodeURIComponent(id)}`);
  return (id) => void navigate({ href: `${logs}?${filter(id)}${failed ? "&failed=true" : ""}&${logsRange(o)}` });
}
