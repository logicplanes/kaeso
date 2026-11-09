import { useState } from "react";
import { Button, CellInset, Dashboard, DashboardCell, Legend, MetricTabs, RankList, Section, Text } from "@logicplanes/kaeso-ui";
import { formatLocale } from "@logicplanes/kaeso-ui/format";
import { bucketLabel, spansYears } from "~/format/buckets";
import { utcDay } from "~/format/period";
import { t } from "~/i18n";
import { projectPath } from "~/shell/pages";
import type { Overview } from "./data";
import { change, METRICS, metricTotal, type MetricId } from "./metrics";
import { DayContext, hourLabel, MetricChart, PREVIOUS, rank, tabRange, useOpenRow, type Ranked } from "./parts";
import { Efficiency, Reliability, Spend } from "./sections";

const TOP = 5;
// German nouns keep their capital inside a sentence.
const inSentence = (label: string) => (formatLocale().startsWith("de") ? label : label.toLowerCase());

const changeOf = (o: Overview, id: MetricId) => {
  if (!o.prevRows) return undefined;
  const c = change(metricTotal(id, o.rows), metricTotal(id, o.prevRows));
  return c === undefined ? undefined : { value: c, better: METRICS[id].better };
};

const perBucket = (o: Overview, metric: string) =>
  t("kaeso.overview.perBucket", "{metric} per {granularity, select, hour {hour} week {week} month {month} other {day}}, UTC", {
    metric,
    granularity: o.step ?? o.g,
  });

/** What a normal bucket looks like and where the highest one is, so one spike reads as a spike. */
function shape(o: Overview, metric: MetricId): string | undefined {
  const m = METRICS[metric];
  const points = o.points.map((p) => ({ at: p.t, v: m.of(p.row) }));
  const peak = points.reduce<(typeof points)[number] | undefined>((a, p) => (p.v > (a?.v ?? 0) ? p : a), undefined);
  if (points.length < 3 || !peak) return undefined;
  const when = o.step ? hourLabel(peak.at) : bucketLabel(utcDay(peak.at), o.g, spansYears(o.from, o.to, o.g));
  if (metric === "errors") return t("kaeso.overview.highest", "Highest: {value}, {when}", { value: m.format(peak.v), when });
  const whole = o.live ? points.slice(0, -1) : points;
  const average = whole.reduce((a, p) => a + p.v, 0) / whole.length;
  return t(
    "kaeso.overview.typical",
    "{average} per {granularity, select, hour {hour} week {week} month {month} other {day}} on average; highest {value}, {when}",
    { average: m.format(metric === "cost" ? average : Math.round(average)), granularity: o.step ?? o.g, value: m.format(peak.v), when },
  );
}

function ChartCell({ o, metric }: { o: Overview; metric: MetricId }) {
  const m = METRICS[metric];
  const what = metric === "errors" ? t("kaeso.overview.errorRate", "Error rate") : m.label();
  const context = o.context && !o.step ? o.context : undefined;
  const note = context ? undefined : shape(o, metric);
  return (
    <DashboardCell
      className="col-span-full"
      title={context ? t("kaeso.overview.lastDays", "{metric} per day, the last 14 days, UTC", { metric: what }) : perBucket(o, what)}
    >
      <CellInset grow>
        <div className="flex flex-wrap items-center justify-between gap-x-24 gap-y-6">
          <Text variant="small" tone="tertiary">
            {note}
          </Text>
          {o.prevRows && !context && (
            <Legend
              items={[
                { id: "now", label: t("kaeso.overview.thisPeriod", "This period"), color: m.color },
                { id: "before", label: o.compareLabel, color: PREVIOUS, dashed: true },
              ]}
            />
          )}
        </div>
        <div id="overview-chart" className="mt-16">
          {context ? <DayContext o={o} context={context} metric={metric} height={280} /> : <MetricChart o={o} id={metric} height={280} />}
        </div>
      </CellInset>
    </DashboardCell>
  );
}

const DIM_TITLE: Record<Ranked, (by: string) => string> = {
  project: (by) => t("kaeso.overview.topProjects", "Projects by {metric}", { metric: by }),
  model: (by) => t("kaeso.overview.topModels", "Models by {metric}", { metric: by }),
  key: (by) => t("kaeso.overview.topKeys", "Keys by {metric}", { metric: by }),
};
const DIM_VIEW: Record<Ranked, string> = { project: "projects", model: "models", key: "keys" };

function TopCell({ o, dim, metric }: { o: Overview; dim: Ranked; metric: MetricId }) {
  const r = rank(o, dim, metric, TOP);
  const title = DIM_TITLE[dim](metric === "errors" ? t("kaeso.overview.failures", "failed requests") : inSentence(METRICS[metric].label()));
  const open = useOpenRow(o, dim, metric === "errors");
  const base = o.project ? `${projectPath(o.org, o.project)}/analytics` : `/orgs/${o.org}/analytics`;
  return (
    <DashboardCell
      className="lg:col-span-1"
      title={title}
      actions={
        <Button size="sm" variant="ghost" href={`${base}/${DIM_VIEW[dim]}?${tabRange(o)}`}>
          {r.count > TOP ? t("kaeso.overview.showAllCount", "Show all {count}", { count: r.count }) : t("kaeso.overview.showAll", "Show all")}
        </Button>
      }
    >
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
    <>
      <Dashboard aria-label={t("kaeso.overview.usage", "Usage in the period")} busy={o.refreshing} columns={2} headingLevel={2}>
        <MetricTabs
          aria-label={t("kaeso.overview.metric", "Metric to chart")}
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
                id === "cost" && o.now.unpriced
                  ? t("kaeso.usage.unpriced", "{count, plural, one {# request} other {# requests}} unknown", { count: o.now.unpriced })
                  : c
                    ? o.compare
                    : o.single && o.live
                      ? t("kaeso.overview.soFarToday", "So far today")
                      : undefined,
            };
          })}
        />
        <ChartCell o={o} metric={metric} />
      </Dashboard>
      <Section title={t("kaeso.overview.top", "Top usage")} description={t("kaeso.overview.topHint", "The five with the most of the total chosen above")}>
        <Dashboard aria-label={t("kaeso.overview.top", "Top usage")} busy={o.refreshing} columns={2}>
          {top.map((d) => (
            <TopCell key={d} o={o} dim={d} metric={metric} />
          ))}
        </Dashboard>
      </Section>
      <Reliability o={o} />
      <Efficiency o={o} />
      {o.showCost && <Spend o={o} />}
    </>
  );
}
