import { lazy, Suspense } from "react";
import { Section, StatGrid, StatTile, TimeseriesChart, VIZ, type TimeSeries } from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { bucketLabel, bucketsBetween, type Granularity } from "~/format/buckets";
import { t } from "~/i18n";

const tokensOf = (r: UsageRow) => r.input_tokens + r.output_tokens + r.cache_read_tokens + r.cache_write_tokens;
export function sumUSD(values: (string | null | undefined)[]): string {
  const micros = values.reduce((acc, v) => acc + (v ? Math.round(Number(v) * 1e6) : 0), 0);
  return (micros / 1e6).toFixed(6).replace(/0+$/, "").replace(/\.$/, ".00");
}

export function periodUSD(amount: string | null | undefined): string {
  if (!amount || !/[1-9]/.test(amount)) return formatUSD("0", { decimals: 2 });
  const n = Number(amount);
  if (n > 0 && n < 0.005) return `<${formatUSD("0.01", { decimals: 2 })}`;
  return formatUSD(amount, { decimals: 2 });
}

export interface UsageTotals {
  requests: number;
  errors: number;
  tokens: number;
  cost: string | null;
}

export function totals(rows: UsageRow[]): UsageTotals {
  const withCost = rows.some((r) => r.cost !== null && r.cost !== undefined);
  return {
    requests: rows.reduce((a, r) => a + r.requests, 0),
    errors: rows.reduce((a, r) => a + r.errors, 0),
    tokens: rows.reduce((a, r) => a + tokensOf(r), 0),
    cost: withCost ? sumUSD(rows.map((r) => r.cost)) : null,
  };
}

export function tileUSD(v: string): string {
  if (!(Number(v) > 0 && Number(v) < 0.01)) return formatUSD(v);
  const rounded = Number(Number(v).toPrecision(2));
  const fixed = rounded.toFixed(1 - Math.floor(Math.log10(rounded))).replace(/0+$/, "");
  return formatUSD(fixed, { decimals: fixed.length - "0.".length });
}
const change = (now: number, before: number | undefined) => (before ? (now - before) / before : undefined);
function errorChange(rate: number, previous: UsageTotals | undefined) {
  const c = previous?.requests ? change(rate, previous.errors / previous.requests) : undefined;
  return c === undefined ? {} : { change: { value: c, better: "down" as const } };
}

export function UsageMetrics({
  rows,
  previous,
  from,
  to,
  granularity = "day",
  compare,
  showCost,
  loading,
}: {
  rows: UsageRow[];
  previous?: UsageRow[] | undefined;
  from: string;
  to: string;
  granularity?: Granularity | undefined;
  compare?: string | undefined;
  showCost: boolean;
  loading?: boolean | undefined;
}) {
  const sum = totals(rows);
  const p = previous && compare ? totals(previous) : undefined;
  const days = byBucket(rows, from, to, granularity);
  const vs = compare ?? "";
  const delta = (now: number, before: number | undefined) => {
    const c = change(now, before);
    if (c !== undefined) return { change: { value: c }, detail: vs };
    return p && now > 0 ? { detail: t("kaeso.usage.upFromNone", "Up from none") } : {};
  };
  const rate = sum.requests ? sum.errors / sum.requests : 0;
  const cost = sum.cost ?? (showCost && rows.length === 0 ? "0" : null);
  const value = (v: string) => (loading ? "—" : v);
  const known = <T extends { trend: number[] }>(props: T): T | { trend: number[] } => (loading ? { trend: [] } : props);
  return (
    <StatGrid aria-label={t("kaeso.usage.inPeriod", "Usage in the period")}>
      <StatTile
        label={t("common.requests", "Requests")}
        value={value(formatNumber(sum.requests))}
        {...known({ ...delta(sum.requests, p?.requests), trend: days.map((d) => d.row?.requests ?? 0) })}
      />
      <StatTile
        label={t("common.tokens", "Tokens")}
        value={value(formatTokens(sum.tokens))}
        {...known({ ...delta(sum.tokens, p?.tokens), trend: days.map((d) => (d.row ? tokensOf(d.row) : 0)) })}
        trendColor={VIZ[1]}
      />
      {showCost && cost !== null && (
        <StatTile
          label={t("common.cost", "Cost")}
          value={value(tileUSD(cost))}
          {...known({ ...delta(Number(cost), p?.cost ? Number(p.cost) : undefined), trend: days.map((d) => Number(d.row?.cost ?? 0)) })}
          trendColor={VIZ[2]}
        />
      )}
      <StatTile
        label={t("kaeso.usage.errors", "Errors")}
        value={value(formatNumber(sum.errors))}
        {...known({
          ...errorChange(rate, p),
          ...(sum.requests ? { detail: t("kaeso.usage.errorRate", "{percent} of requests", { percent: formatPercent(rate) }) } : {}),
          trend: days.map((d) => d.row?.errors ?? 0),
        })}
        trendColor={VIZ[3]}
      />
    </StatGrid>
  );
}

export interface DailyProps {
  rows: UsageRow[];
  from: string;
  to: string;
  granularity?: Granularity | undefined;
  loading?: boolean | undefined;
  failed?: boolean | undefined;
  onRetry?: (() => void) | undefined;
}

export function byBucket(rows: UsageRow[], from: string, to: string, g: Granularity = "day") {
  const map = new Map(rows.map((r) => [r.key, r]));
  return bucketsBetween(from, to, g).map((day) => ({ day, row: map.get(day) }));
}

const pointTime = (day: string) => {
  const [y = 1970, m = 1, d = 1] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const timeNames = (days: { day: string }[], g: Granularity) => {
  if (g === "day") return {};
  const names = new Map(days.map((d) => [pointTime(d.day).getTime(), bucketLabel(d.day, g)]));
  return { formatTime: (time: Date) => names.get(time.getTime()) ?? "" };
};

export const tokensByKind = (g: Granularity) =>
  t("kaeso.usage.tokensByKind", "Tokens by kind per {granularity, select, week {week} month {month} other {day}}", { granularity: g });

export const chartState = ({ loading, failed, onRetry }: Pick<DailyProps, "loading" | "failed" | "onRetry">) => ({
  loading,
  error: failed && t("kaeso.usage.chartFailed", "Couldn't load this chart"),
  onRetry,
});

const bars = () => import("./usage-bars");
const TokenBars = lazy(() => bars().then((m) => ({ default: m.TokenBars })));
const DayBars = lazy(() => bars().then((m) => ({ default: m.DayBars })));
const waiting = (label: string) => <TimeseriesChart aria-label={label} series={[]} height={240} loading />;

export function RequestsChart(props: DailyProps) {
  const g = props.granularity ?? "day";
  const days = byBucket(props.rows, props.from, props.to, g);
  const label = t("kaeso.usage.requestsPer", "Requests and errors per {granularity, select, week {week} month {month} other {day}}", { granularity: g });
  const series: TimeSeries[] = [
    { id: "requests", label: t("common.requests", "Requests"), color: VIZ[0], points: days.map((d) => ({ t: pointTime(d.day), v: d.row?.requests ?? 0 })) },
    { id: "errors", label: t("kaeso.usage.errors", "Errors"), color: VIZ[3], points: days.map((d) => ({ t: pointTime(d.day), v: d.row?.errors ?? 0 })) },
  ];
  return (
    <Section
      title={t("common.requests", "Requests")}
      description={t("kaeso.usage.requestsPerUtc", "Requests and errors per {granularity, select, week {week} month {month} other {day}} (UTC)", {
        granularity: g,
      })}
    >
      {days.length === 1 ? (
        <Suspense fallback={waiting(label)}>
          <DayBars {...props} label={label} days={days} series={series} />
        </Suspense>
      ) : (
        <TimeseriesChart
          aria-label={label}
          series={series}
          kind="area"
          height={240}
          formatValue={formatNumber}
          empty={props.rows.length === 0}
          emptyText={t("kaeso.usage.noRequests", "No requests in this period.")}
          {...timeNames(days, g)}
          {...chartState(props)}
        />
      )}
    </Section>
  );
}

export function TokensChart(props: DailyProps) {
  const g = props.granularity ?? "day";
  return (
    <Section
      title={t("common.tokens", "Tokens")}
      description={t("kaeso.usage.tokensPerUtc", "By kind, per {granularity, select, week {week} month {month} other {day}} (UTC)", { granularity: g })}
    >
      <Suspense fallback={waiting(tokensByKind(g))}>
        <TokenBars {...props} />
      </Suspense>
    </Section>
  );
}

export function CostChart(props: DailyProps) {
  const g = props.granularity ?? "day";
  const days = byBucket(props.rows, props.from, props.to, g);
  const series: TimeSeries[] = [
    { id: "cost", label: t("common.cost", "Cost"), color: VIZ[0], points: days.map((d) => ({ t: pointTime(d.day), v: Number(d.row?.cost ?? 0) })) },
  ];
  return (
    <Section
      title={t("common.cost", "Cost")}
      description={t("kaeso.usage.costPerUtc", "USD per {granularity, select, week {week} month {month} other {day}} (UTC), at the prices of request time", {
        granularity: g,
      })}
    >
      <TimeseriesChart
        aria-label={t("kaeso.usage.costPer", "Cost per {granularity, select, week {week} month {month} other {day}}", { granularity: g })}
        series={series}
        kind="line"
        height={200}
        hideLegend
        formatValue={(v) => formatUSD(v.toFixed(6))}
        empty={props.rows.length === 0}
        emptyText={t("kaeso.usage.noCost", "No cost in this period.")}
        {...timeNames(days, g)}
        {...chartState(props)}
      />
    </Section>
  );
}
