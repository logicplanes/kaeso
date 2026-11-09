import { lazy, Suspense, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { CellInset, Dashboard, DashboardCell, Readout, Segmented, Text, type Change } from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { orgUsageQuery, projectUsageQuery, timelineQuery } from "~/api/queries/usage";
import { useRequestLogs } from "~/auth/session";
import { dayFormat, dayLabel, shiftDay, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { byBucket, periodUSD, totals } from "~/kaeso/usage-panels";
import { cacheHit, change, forecast, METRICS, metricTotal, type MetricId } from "./metrics";
import { fill, iso, useMinute, utcTimeFormat } from "./timeline";

// The trends draw only once their numbers arrive, so their chart code loads beside the data instead of with the page.
const Sparkline = lazy(() => import("@logicplanes/kaeso-ui/charts/sparkline").then((m) => ({ default: m.Sparkline })));

const monthName = dayFormat({ month: "long" });
const hourName = utcTimeFormat({ weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

type Window = "7d" | "14d" | "30d";
const WINDOW_DAYS: Record<Window, number> = { "7d": 7, "14d": 14, "30d": 30 };

function TrendCell({
  title,
  href,
  value,
  change,
  detail,
  trend,
  labels,
  format,
  color,
  loading,
}: {
  title: string;
  href: string;
  value: string;
  change: Change | undefined;
  detail?: string | undefined;
  trend: number[];
  labels: string[];
  format: (v: number) => string;
  color: string;
  loading: boolean;
}) {
  return (
    <DashboardCell className="lg:col-span-1" title={title} href={href}>
      <Readout value={value} {...(change ? { change } : {})} {...(detail ? { detail } : {})} loading={loading} />
      <CellInset grow>
        <div className="mt-auto flex flex-col">
          <div className="mt-16">
            {!loading && trend.length > 1 && (
              <Suspense fallback={null}>
                <Sparkline
                  values={trend}
                  fluid
                  height={48}
                  color={color}
                  labels={labels}
                  seriesLabel={title}
                  formatValue={format}
                  aria-label={t("kaeso.home.perDay", "{metric} per day", { metric: title })}
                />
              </Suspense>
            )}
          </div>
        </div>
      </CellInset>
    </DashboardCell>
  );
}

export function HomeMetrics({
  org,
  project,
  canUsage,
  showCost,
  analytics,
  monthExtra,
}: {
  org: string;
  project?: string | undefined;
  canUsage: boolean;
  showCost: boolean;
  analytics: string;
  monthExtra?: ReactNode;
}) {
  const [range, setRange] = useState<Window>("14d");
  const today = utcDay(new Date());
  const days = WINDOW_DAYS[range];
  const from = shiftDay(today, -(days - 1));
  const before = { from: shiftDay(from, -days), to: shiftDay(from, -1) };
  const monthStart = `${today.slice(0, 8)}01`;
  const usage = (f: string, to: string) => (project ? projectUsageQuery(org, project, f, to, "day") : orgUsageQuery(org, f, to, "day"));
  const now = useQuery({ ...usage(from, today), enabled: canUsage, placeholderData: (p) => p, refetchInterval: 60_000 });
  const prev = useQuery({ ...usage(before.from, before.to), enabled: canUsage, placeholderData: (p) => p });
  const month = useQuery({ ...usage(monthStart, today), enabled: canUsage && showCost, refetchInterval: 60_000 });
  const logsOn = useRequestLogs();
  const minute = useMinute();
  const fine = useQuery({
    ...timelineQuery(org, project, iso(Date.parse(`${from}T00:00:00Z`)), iso(minute + 60_000), "hour"),
    enabled: canUsage && logsOn,
    placeholderData: (p) => p,
    retry: false,
    refetchInterval: 60_000,
  });
  if (!canUsage) return null;
  const daily = byBucket(now.data ?? [], from, today);
  const startMs = Date.parse(`${from}T00:00:00Z`);
  const hourly = fill(fine.data, startMs, minute + 60_000, "hour");
  const points = fine.data ? hourly : daily.map((d) => ({ t: new Date(`${d.day}T00:00:00Z`), row: d.row }));
  const series = (f: (r: UsageRow | undefined) => number) => points.map((p) => f(p.row));
  const labels = points.map((p) => (fine.data ? hourName(p.t) : dayLabel(utcDay(p.t))));
  const all = now.data ?? [];
  const was = prev.data ?? [];
  const delta = (id: MetricId) => (was.some((r) => r.requests > 0) ? change(metricTotal(id, all), metricTotal(id, was)) : undefined);
  const ch = (id: MetricId) => {
    const c = delta(id);
    return c === undefined ? undefined : { value: c, better: METRICS[id].better };
  };
  const vs = t("kaeso.home.vsBefore", "vs. the {days} days before", { days });
  const window = range === "14d" ? "30d" : range;
  const link = (metric?: MetricId) => `${analytics}?period=${window}${metric ? `&metric=${metric}` : ""}`;
  const loading = now.isPending;
  const s = totals(all);
  const hit = cacheHit(all);
  const spent = totals(month.data ?? []).cost ?? "0";
  const ahead = month.data ? forecast(month.data, monthStart, today) : undefined;
  return (
    <>
      <div className="mb-12 flex items-center justify-between gap-12">
        <Text as="h2" variant="title-sm">
          {t("kaeso.home.activity", "Activity")}
        </Text>
        <Segmented
          aria-label={t("kaeso.home.window", "Days shown")}
          size="sm"
          value={range}
          onValueChange={(v) => setRange(v as Window)}
          options={(["7d", "14d", "30d"] as const).map((w) => ({ value: w, label: t("kaeso.home.days", "{count}d", { count: WINDOW_DAYS[w] }) }))}
        />
      </div>
      <Dashboard aria-label={t("kaeso.home.activity", "Activity")} columns={3} busy={now.isPlaceholderData}>
        <TrendCell
          title={METRICS.requests.label()}
          href={link("requests")}
          value={formatNumber(s.requests)}
          change={ch("requests")}
          detail={ch("requests") ? vs : undefined}
          trend={series((r) => r?.requests ?? 0)}
          labels={labels}
          format={METRICS.requests.format}
          color={METRICS.requests.color}
          loading={loading}
        />
        <TrendCell
          title={METRICS.tokens.label()}
          href={link("tokens")}
          value={METRICS.tokens.format(s.tokens)}
          change={ch("tokens")}
          detail={ch("tokens") ? vs : undefined}
          trend={series(METRICS.tokens.of)}
          labels={labels}
          format={METRICS.tokens.format}
          color={METRICS.tokens.color}
          loading={loading}
        />
        <TrendCell
          title={METRICS.cache.label()}
          href={link()}
          value={hit === null ? "—" : METRICS.cache.format(hit)}
          change={ch("cache")}
          detail={ch("cache") ? vs : undefined}
          trend={series(METRICS.cache.of)}
          labels={labels}
          format={METRICS.cache.format}
          color={METRICS.cache.color}
          loading={loading}
        />
        {showCost && (
          <DashboardCell className="lg:col-span-1" title={t("kaeso.home.monthSpend", "{month} spend", { month: monthName(monthStart) })} href={link("cost")}>
            <Readout
              value={periodUSD(spent)}
              {...(ahead ? { detail: t("project.home.onPace", "On pace for {amount}", { amount: formatUSD(ahead.projected.toFixed(2)) }) } : {})}
              loading={month.isPending}
            />
            {monthExtra && (
              <CellInset grow>
                <div className="mt-auto flex flex-col">
                  <div className="mt-16">{monthExtra}</div>
                </div>
              </CellInset>
            )}
          </DashboardCell>
        )}
        {showCost && (
          <TrendCell
            title={METRICS.cost.label()}
            href={link("cost")}
            value={periodUSD(s.cost ?? "0")}
            change={ch("cost")}
            detail={
              s.unpriced
                ? t("kaeso.usage.unpriced", "{count, plural, one {# request} other {# requests}} unknown", { count: s.unpriced })
                : ch("cost")
                  ? vs
                  : undefined
            }
            trend={series(METRICS.cost.of)}
            labels={labels}
            format={METRICS.cost.format}
            color={METRICS.cost.color}
            loading={loading}
          />
        )}
        <TrendCell
          title={t("kaeso.overview.failed", "Failed requests")}
          href={link("errors")}
          value={formatNumber(s.errors)}
          change={ch("errors")}
          detail={t("kaeso.overview.ofRequests", "{rate} of requests", { rate: formatPercent(s.requests ? s.errors / s.requests : 0) })}
          trend={series((r) => r?.errors ?? 0)}
          labels={labels}
          format={METRICS.requests.format}
          color={METRICS.errors.color}
          loading={loading}
        />
      </Dashboard>
    </>
  );
}
