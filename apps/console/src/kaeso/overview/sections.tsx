import { useNavigate } from "@tanstack/react-router";
import { BarChart, Button, CellInset, Dashboard, DashboardCell, RankList, Readout, ShareBar, Text, UsageBreakdown, type RankItem } from "@logicplanes/kaeso-ui";
import { formatDuration, formatNumber, formatPercent, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { bucketLabel, spansYears } from "~/format/buckets";
import { errorKindLabel } from "~/format/labels";
import { dayFormat, utcDay } from "~/format/period";
import { t } from "~/i18n";
import { projectPath } from "~/shell/pages";
import { DayContext, hourLabel, logsRange, MetricChart, rank, useOpenRow } from "./parts";
import type { Overview } from "./data";
import { Limit } from "./limit";
import { cacheHit, change, METRICS } from "./metrics";
import { utcTimeFormat } from "./timeline";

const monthName = dayFormat({ month: "long" });
const hourOf = utcTimeFormat({ day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const ROWS = 5;
// A 95th percentile of a handful of requests is one slow request, not a slow hour.
const BUSY_HOUR = 10;

function FailedCell({ o, className }: { o: Overview; className: string }) {
  const title = t("kaeso.overview.failed", "Failed requests");
  const withYear = spansYears(o.from, o.to, o.g);
  const before = o.prevRows?.reduce((a, r) => a + r.errors, 0);
  const moved = change(o.now.errors, before);
  return (
    <DashboardCell
      className={className}
      title={title}
      {...(o.project && o.showErrors ? { href: `${projectPath(o.org, o.project)}/logs?failed=true&${logsRange(o)}` } : {})}
    >
      <Readout
        value={formatNumber(o.now.errors)}
        {...(moved === undefined ? {} : { change: { value: moved, better: "down" as const } })}
        detail={t("kaeso.overview.ofRequests", "{rate} of requests", { rate: formatPercent(o.now.requests ? o.now.errors / o.now.requests : 0) })}
        loading={o.loading}
      />
      <CellInset grow>
        <div className="mt-auto flex flex-col">
          <div className="mt-16">
            <BarChart
              aria-label={t("kaeso.overview.failedPerBucket", "Failed requests over the period")}
              categories={o.points.map((p) => (o.step ? hourLabel(p.t) : bucketLabel(utcDay(p.t), o.g, withYear)))}
              dates={o.points.map((p) => p.t)}
              series={[{ id: "failed", label: title, color: METRICS.errors.color, values: o.points.map((p) => p.row?.errors ?? 0) }]}
              shares={false}
              height={132}
              formatValue={formatNumber}
              loading={o.pointsLoading}
              error={o.daily.isError && t("kaeso.usage.chartFailed", "Couldn't load this chart")}
              onRetry={() => void o.daily.refetch()}
              empty={!o.daily.isPending && o.now.errors === 0}
              emptyText={t("project.overview.noErrors", "No failed requests.")}
            />
          </div>
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function KindsCell({ o, failures }: { o: Overview; failures: NonNullable<Overview["failures"]> }) {
  const navigate = useNavigate();
  const title = t("kaeso.overview.failedByKind", "Failures by kind");
  const counts = new Map<string, number>();
  for (const b of failures.data ?? []) counts.set(b.key ?? "", (counts.get(b.key ?? "") ?? 0) + b.requests);
  const total = [...counts.values()].reduce((a, n) => a + n, 0) || 1;
  const items: RankItem[] = [...counts]
    .sort((a, b) => b[1] - a[1])
    .slice(0, ROWS)
    .map(([kind, n]) => ({
      id: kind,
      label: <span className="min-w-0 truncate">{kind ? errorKindLabel(kind) : t("kaeso.overview.otherFailures", "Other failures")}</span>,
      secondary: formatPercent(n / total, 0),
      value: formatNumber(n),
      share: n / total,
    }));
  const logs = o.project && o.showErrors ? `${projectPath(o.org, o.project)}/logs` : undefined;
  return (
    <DashboardCell className="lg:col-span-1" title={title}>
      <RankList
        aria-label={title}
        items={items}
        loading={failures.isPending}
        rows={ROWS}
        empty={t("project.overview.noErrors", "No failed requests.")}
        {...(logs
          ? { onOpen: (kind: string) => void navigate({ href: `${logs}?failed=true${kind ? `&error=${encodeURIComponent(kind)}` : ""}&${logsRange(o)}` }) }
          : {})}
      />
    </DashboardCell>
  );
}

function FailedByCell({ o }: { o: Overview }) {
  const dim = o.project ? "model" : "project";
  const title = o.project ? t("kaeso.overview.failedByModel", "Failed requests by model") : t("kaeso.overview.failedByProject", "Failed requests by project");
  const r = rank(o, dim, "errors", ROWS);
  const open = useOpenRow(o, dim, true);
  return (
    <DashboardCell className="lg:col-span-1" title={title}>
      <RankList
        aria-label={title}
        items={r.items}
        loading={r.loading}
        rows={ROWS}
        empty={t("project.overview.noErrors", "No failed requests.")}
        {...(open ? { onOpen: open } : {})}
      />
    </DashboardCell>
  );
}

function SlowestCell({ o, hours }: { o: Overview; hours: NonNullable<Overview["hours"]> }) {
  const navigate = useNavigate();
  const title = t("kaeso.overview.slowest", "Slowest hours, 95th percentile");
  // An hour of only observed requests whose source sends no timing has no latency.
  const all = (hours.data ?? []).flatMap((b) => (b.duration_p95_ms == null ? [] : [{ ...b, p95: b.duration_p95_ms }]));
  const busy = all.filter((b) => b.requests >= BUSY_HOUR);
  const items: RankItem[] = [...(busy.length ? busy : all)]
    .sort((a, b) => b.p95 - a.p95)
    .slice(0, ROWS)
    .map((b) => ({
      id: b.start,
      label: <span className="min-w-0 truncate">{hourOf(new Date(b.start))}</span>,
      secondary: t("kaeso.analytics.ofRequests", "{requests} requests", { requests: formatNumber(b.requests) }),
      value: formatDuration(b.p95),
    }));
  const logs = o.project && o.showErrors ? `${projectPath(o.org, o.project)}/logs` : undefined;
  // Every listed hour is one of these.
  const p95 = new Map(all.map((b) => [b.start, b.p95]));
  return (
    <DashboardCell className="lg:col-span-1" title={title}>
      <RankList
        aria-label={title}
        items={items}
        loading={hours.isPending}
        rows={ROWS}
        empty={t("kaeso.overview.nothing", "Nothing in this period")}
        {...(logs
          ? {
              // That day's requests at least as slow as the hour's 95th percentile.
              onOpen: (start: string) => {
                const day = utcDay(new Date(start));
                void navigate({ href: `${logs}?from=${day}&to=${day}&utc=true&min_duration_ms=${String(p95.get(start))}` });
              },
            }
          : {})}
      />
    </DashboardCell>
  );
}

export function Reliability({ o }: { o: Overview }) {
  return (
    <Dashboard aria-label={t("kaeso.overview.reliability", "Reliability")} busy={o.refreshing} columns={2} {...(o.hours ? { wideColumns: 3 as const } : {})}>
      <FailedCell o={o} className={o.hours ? "lg:col-span-2 xl:col-span-1" : "lg:col-span-1"} />
      {o.failures ? <KindsCell o={o} failures={o.failures} /> : <FailedByCell o={o} />}
      {o.hours && <SlowestCell o={o} hours={o.hours} />}
    </Dashboard>
  );
}

function CacheCell({ o }: { o: Overview }) {
  const hit = cacheHit(o.rows);
  const before = o.prevRows ? cacheHit(o.prevRows) : null;
  const moved = hit === null ? undefined : change(hit, before ?? undefined);
  const read = o.rows.reduce((a, r) => a + r.cache_read_tokens, 0);
  return (
    <DashboardCell className="lg:col-span-1" title={t("kaeso.overview.cacheHit", "Cache hit")}>
      <Readout
        value={hit === null ? "—" : formatPercent(hit, 0)}
        {...(moved === undefined ? {} : { change: { value: moved, better: "up" as const } })}
        detail={t("kaeso.overview.cacheRead", "{tokens} input tokens from cache", { tokens: formatTokens(read) })}
        loading={o.loading}
      />
      <CellInset grow>
        <div className="mt-auto flex flex-col">
          <div className="mt-16">
            {o.context && !o.step ? (
              <DayContext o={o} context={o.context} metric="cache" height={132} />
            ) : (
              <MetricChart o={o} id="cache" height={132} previous={false} />
            )}
          </div>
        </div>
      </CellInset>
    </DashboardCell>
  );
}

function TokenCell({ o }: { o: Overview }) {
  const sum = (f: (r: UsageRow) => number) => o.rows.reduce((a, r) => a + f(r), 0);
  const n = o.now.requests || 1;
  const input = sum((r) => r.input_tokens + r.cache_read_tokens + r.cache_write_tokens);
  return (
    <DashboardCell className="lg:col-span-1" title={t("kaeso.overview.tokenMix", "Tokens per request")}>
      <Readout
        value={formatTokens(Math.round(o.now.tokens / n))}
        detail={t("kaeso.overview.inOut", "{input} in, {output} out", {
          input: formatTokens(Math.round(input / n)),
          output: formatTokens(Math.round(sum((r) => r.output_tokens) / n)),
        })}
        loading={o.loading}
      />
      <CellInset>
        <div className="flex flex-col">
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
        </div>
      </CellInset>
    </DashboardCell>
  );
}

export const efficiencyTitle = (o: Pick<Overview, "showCost">) =>
  o.showCost ? t("kaeso.overview.costEfficiency", "Cost and efficiency") : t("kaeso.overview.efficiency", "Efficiency");

export function Efficiency({ o }: { o: Overview }) {
  return (
    <Dashboard aria-label={efficiencyTitle(o)} busy={o.refreshing} columns={2}>
      <CacheCell o={o} />
      <TokenCell o={o} />
      {o.showCost && <ForecastCell o={o} />}
      {o.showCost && <CostSplitCell o={o} />}
    </Dashboard>
  );
}

function ForecastCell({ o }: { o: Overview }) {
  const month = o.forecast ? monthName(o.forecast.month) : monthName(utcDay(new Date()));
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={t("kaeso.overview.forecast", "Forecast for {month}", { month })}
      actions={
        o.project &&
        o.can("limits.view") && (
          <Button size="sm" variant="ghost" href={`${projectPath(o.org, o.project)}/limits`}>
            {t("kaeso.overview.limits", "Limits")}
          </Button>
        )
      }
    >
      <Readout
        value={o.forecast ? o.money.period(o.forecast.projected.toFixed(2)) : "—"}
        {...(o.forecast
          ? { detail: t("kaeso.overview.soFar", "{amount} so far, at the last 7 days' pace", { amount: o.money.period(o.forecast.soFar.toFixed(2)) }) }
          : {})}
        loading={o.forecastLoading}
      />
      {o.spend && (
        <CellInset>
          <div className="mt-20">
            <Limit limit={o.spend} />
          </div>
        </CellInset>
      )}
    </DashboardCell>
  );
}

function CostSplitCell({ o }: { o: Overview }) {
  const q = o.project ? o.grouped.model : o.grouped.project;
  const priced = [...(q?.data ?? [])].filter((r) => Number(r.converted_cost ?? 0) > 0).sort((a, b) => Number(b.converted_cost) - Number(a.converted_cost));
  const top = priced.slice(0, 4).map((r) => ({
    id: r.key,
    label: o.project ? o.modelName(r.key, r.name) : (o.projectOf(r.key)?.name ?? r.key),
    value: Number(r.converted_cost),
  }));
  const rest = priced.slice(4).reduce((a, r) => a + Number(r.converted_cost), 0);
  const slices = rest > 0 ? [...top, { id: "other", label: t("kaeso.overview.other", "Other"), value: rest, color: "var(--k-fill-3)" }] : top;
  const title = o.project ? t("kaeso.overview.costByModel", "Cost by model") : t("kaeso.overview.costByProject", "Cost by project");
  return (
    <DashboardCell className="lg:col-span-1" title={title}>
      <CellInset>
        <div className="mt-8 flex flex-col gap-12">
          {slices.length > 0 ? (
            <ShareBar aria-label={title} slices={slices} formatValue={(v) => o.money.period(v.toFixed(6))} />
          ) : (
            !q?.isPending && (
              <Text variant="small" tone="tertiary">
                {t("kaeso.overview.noCost", "No cost in this period")}
              </Text>
            )
          )}
          {o.now.unpriced > 0 && (
            <Text as="p" variant="caption" tone="tertiary">
              {t(
                "kaeso.analytics.costShareUnpriced",
                "{count, plural, one {# request without a price isn't included.} other {# requests without a price aren't included.}}",
                { count: o.now.unpriced },
              )}
            </Text>
          )}
        </div>
      </CellInset>
    </DashboardCell>
  );
}
