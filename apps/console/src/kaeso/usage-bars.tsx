import { BarChart, TOKEN_KIND_ORDER, tokenKindColor, type TimeSeries } from "@logicplanes/kaeso-ui";
import { formatNumber, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { t } from "~/i18n";
import { bucketNames, byBucket, chartState, tokensByKind, type DailyProps } from "./usage-panels";

export function DayBars(props: DailyProps & { label: string; days: { day: string }[]; series: TimeSeries[] }) {
  const names = bucketNames(props.days, props);
  return (
    <BarChart
      aria-label={props.label}
      categories={props.days.map((d) => names.label(d.day))}
      dates={props.days.map((d) => d.day)}
      series={props.series.map((s) => ({ id: s.id, label: s.label, color: s.color, values: s.points.map((p) => p.v) }))}
      layout="grouped"
      shares={false}
      height={240}
      formatValue={formatNumber}
      empty={props.rows.length === 0}
      emptyText={t("kaeso.usage.noRequests", "No requests in this period.")}
      {...chartState(props)}
    />
  );
}

export function TokenBars(props: DailyProps) {
  const g = props.granularity ?? "day";
  const days = byBucket(props.rows, props.from, props.to, g);
  const names = bucketNames(days, props);
  const field: Record<(typeof TOKEN_KIND_ORDER)[number], (r: UsageRow) => number> = {
    input: (r) => r.input_tokens,
    cacheRead: (r) => r.cache_read_tokens,
    cacheWrite: (r) => r.cache_write_tokens,
    output: (r) => r.output_tokens - r.reasoning_tokens,
    reasoning: (r) => r.reasoning_tokens,
  };
  const label: Record<(typeof TOKEN_KIND_ORDER)[number], string> = {
    input: t("kaeso.usage.input", "Input"),
    cacheRead: t("kaeso.usage.cacheRead", "Cache read"),
    cacheWrite: t("kaeso.usage.cacheWrite", "Cache write"),
    output: t("kaeso.usage.output", "Output"),
    reasoning: t("kaeso.usage.reasoning", "Reasoning"),
  };
  const series = TOKEN_KIND_ORDER.map((k) => ({
    id: k,
    label: label[k],
    color: tokenKindColor(k),
    values: days.map((d) => (d.row ? Math.max(0, field[k](d.row)) : 0)),
  }));
  return (
    <BarChart
      aria-label={tokensByKind(g)}
      categories={days.map((d) => names.label(d.day))}
      dates={days.map((d) => d.day)}
      axisLabels={days.map((d) => names.axis(d.day))}
      partial={names.partial}
      series={series}
      layout="stacked"
      height={240}
      formatValue={formatTokens}
      empty={props.rows.length === 0}
      emptyText={t("kaeso.usage.noTokens", "No tokens in this period.")}
      {...chartState(props)}
    />
  );
}
