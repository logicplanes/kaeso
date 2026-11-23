import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AppFrame,
  Badge,
  Button,
  CellInset,
  Dashboard,
  DashboardCell,
  Heading,
  Legend,
  Link,
  Meter,
  MetricTabs,
  RankList,
  Skeleton,
  Text,
  TimeseriesChart,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { moneyIn, type Money } from "~/format/currency";
import { isApiError } from "~/api/errors";
import type { SharedBreakdown, SharedLimit, SharedRow, SharedView, UsageRow } from "~/api/generated/types";
import { sharedViewQuery } from "~/api/queries/shared-links";
import { clientLabel } from "~/format/clients";
import { fullDayLabel, utcDay } from "~/format/period";
import { dialectLabel, limitKindLabel, periodLabel } from "~/format/labels";
import { limitAmount, limitUse } from "~/format/limit-use";
import { t } from "~/i18n";
import { LimitFigure } from "~/kaeso/limit-figure";
import { change, METRICS, metricTotal, type MetricId } from "~/kaeso/overview/metrics";
import { PREVIOUS } from "~/kaeso/overview/parts";
import { useMinute } from "~/kaeso/overview/timeline";
import { PublicPage } from "~/shell/public-page";
import { usePageTitle } from "~/shell/title";

const DAY = 86_400_000;

/** A shared row as the usage panels count it. */
export function asUsage(r: SharedRow, currency: string): UsageRow {
  return {
    key: r.key ?? "",
    requests: r.requests,
    errors: r.errors,
    input_tokens: r.input_tokens,
    output_tokens: r.output_tokens,
    cache_read_tokens: r.cache_read_tokens,
    cache_write_tokens: r.cache_write_tokens,
    reasoning_tokens: r.reasoning_tokens,
    cost: r.cost ?? null,
    converted_cost: r.converted_cost ?? null,
    currency,
    tool_calls: r.tool_calls,
    tool_call_errors: 0,
    unpriced: 0,
    media: { images: 0, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 },
  };
}

function dayList(from: string, to: string): string[] {
  const out: string[] = [];
  for (let at = Date.parse(`${from}T00:00:00Z`); at <= Date.parse(`${to}T00:00:00Z`); at += DAY) out.push(new Date(at).toISOString().slice(0, 10));
  return out;
}

/** What a breakdown row is called on the page: its model, route or client, its name, or its place when the link hides names. */
export function rowLabel(by: string, r: SharedRow): string {
  if (r.key === "playground") return t("kaeso.share.playground", "Playground");
  if (r.name) return r.name;
  if (r.number) {
    if (r.key === "source") return t("kaeso.share.sourceNumber", "Source {n}", { n: r.number });
    if (by === "member") return t("kaeso.share.memberNumber", "Member {n}", { n: r.number });
    if (by === "key") return t("kaeso.share.keyNumber", "Key {n}", { n: r.number });
    return t("kaeso.share.projectNumber", "Project {n}", { n: r.number });
  }
  const key = r.key ?? "";
  if (by === "client") return clientLabel(key);
  if (by === "route") {
    const [from = "", to = "", provider = ""] = key.split(":");
    return t("kaeso.share.route", "{from} to {to} at {provider}", { from: dialectLabel(from), to: dialectLabel(to), provider });
  }
  return key || t("kaeso.share.unknown", "Unknown");
}

const BREAKDOWN_TITLE: Record<string, () => string> = {
  model: () => t("kaeso.share.byModel", "Models"),
  key: () => t("kaeso.share.byKey", "Keys"),
  member: () => t("kaeso.share.byMember", "Members"),
  route: () => t("kaeso.share.byRoute", "Routes"),
  project: () => t("kaeso.share.byProject", "Projects"),
  client: () => t("kaeso.share.byClient", "Clients"),
};

export function SharedPage({ token }: { token: string }) {
  const view = useQuery(sharedViewQuery(token));
  usePageTitle(view.data?.title ?? t("kaeso.share.pageTitle", "Shared usage"));
  if (view.isPending) {
    return (
      <Frame>
        <Skeleton className="w-320" />
        <Skeleton className="w-full" />
      </Frame>
    );
  }
  if (view.isError) {
    const signIn = isApiError(view.error) && view.error.status === 401;
    return signIn ? (
      <PublicPage
        title={t("kaeso.share.membersOnly", "This link is for an organization's members")}
        description={t("kaeso.share.membersOnlyHint", "Sign in with an account in the organization that shared it.")}
      >
        <Button size="xl" variant="primary" fullWidth href={`/sign-in?next=${encodeURIComponent(`/share/${token}`)}`}>
          {t("kaeso.share.signIn", "Sign in")}
        </Button>
      </PublicPage>
    ) : (
      <PublicPage
        title={t("kaeso.share.gone", "This link doesn't work")}
        description={t("kaeso.share.goneHint", "It may have expired or been revoked; ask whoever shared it for a new one.")}
      />
    );
  }
  return <SharedBody v={view.data} />;
}

function Frame({ children }: { children: ReactNode }) {
  return (
    <main className="flex h-dvh flex-col">
      <AppFrame.Body padding="page">
        <div className="flex w-full flex-col gap-24">{children}</div>
      </AppFrame.Body>
    </main>
  );
}

function subtitle(v: SharedView): string {
  const period = v.from === v.to ? fullDayLabel(v.from) : t("kaeso.share.period", "{from} to {to}", { from: fullDayLabel(v.from), to: fullDayLabel(v.to) });
  return [v.organization, v.project, v.member, period].filter(Boolean).join(" · ");
}

function SharedBody({ v }: { v: SharedView }) {
  return (
    <Frame>
      <header className="flex flex-col gap-8">
        <div className="flex items-baseline gap-8">
          <Heading level={1} className="min-w-0">
            {v.title}
          </Heading>
          <Badge tone="outline" className="shrink-0">
            {v.live ? t("kaeso.share.liveBadge", "Live") : t("kaeso.share.snapshotBadge", "Snapshot")}
          </Badge>
        </div>
        <Text tone="secondary">{subtitle(v)}</Text>
      </header>
      {v.kind === "budget" ? <Budgets v={v} /> : <Usage v={v} />}
      <footer className="flex flex-wrap items-baseline justify-between gap-x-16 gap-y-8">
        <Text variant="caption" tone="tertiary">
          {v.expires_at
            ? t("kaeso.share.readOnlyUntil", "A read-only view shared from Kaeso, until {date}.", { date: formatDate(v.expires_at) })
            : t("kaeso.share.readOnly", "A read-only view shared from Kaeso.")}
        </Text>
        {v.made_with_kaeso && (
          <Text variant="caption" tone="secondary">
            <Link href="https://kaeso.app" variant="action" target="_blank" rel="noopener noreferrer">
              {t("kaeso.share.madeWith", "Made with Kaeso")}
            </Link>
          </Text>
        )}
      </footer>
    </Frame>
  );
}

function Usage({ v }: { v: SharedView }) {
  const rows = v.days.map((r) => asUsage(r, v.currency));
  const before = v.previous_days.map((r) => asUsage(r, v.currency));
  const money = moneyIn(v.currency);
  const ids: MetricId[] = ["requests", "tokens", ...(v.costs ? (["cost"] as const) : []), "errors", "cache"];
  const initial = ids.includes(v.metric as MetricId) ? (v.metric as MetricId) : "requests";
  const [metric, setMetric] = useState<MetricId>(initial);
  const m = METRICS[metric];
  const days = dayList(v.from, v.to);
  const point = (list: UsageRow[], day: string) => list.find((r) => r.key === day);
  const shift = days.length * DAY;
  const breakdowns = v.kind === "project_card" ? v.breakdowns.map((b) => ({ ...b, rows: b.rows.slice(0, 5) })) : v.breakdowns;
  return (
    <>
      <Dashboard aria-label={t("kaeso.share.usage", "Usage in the period")} columns={2} headingLevel={2}>
        <MetricTabs
          aria-label={t("kaeso.share.metric", "Metric to chart")}
          controls="shared-chart"
          value={metric}
          onValueChange={(id) => setMetric(id as MetricId)}
          items={ids.map((id) => {
            const c = change(metricTotal(id, rows), metricTotal(id, before));
            return {
              id,
              label: METRICS[id].label(),
              value: METRICS[id].format(metricTotal(id, rows), money),
              ...(c === undefined ? {} : { change: { value: c, better: METRICS[id].better }, detail: t("kaeso.share.vsBefore", "vs. the days before") }),
            };
          })}
        />
        <DashboardCell className="lg:col-span-2" title={m.label()}>
          <CellInset>
            <Legend
              items={[
                { id: "now", label: t("kaeso.share.thisPeriod", "This period"), color: m.color },
                { id: "before", label: t("kaeso.share.before", "The days before"), color: PREVIOUS, dashed: true },
              ]}
            />
            <div id="shared-chart" className="mt-16">
              <TimeseriesChart
                aria-label={m.label()}
                kind="area"
                height={280}
                hideLegend
                shares={false}
                formatValue={(value) => m.format(value, money)}
                timeZone="UTC"
                tickAt="points"
                formatTime={(time: Date) => fullDayLabel(utcDay(time))}
                empty={rows.length === 0}
                emptyText={t("kaeso.share.noRequests", "No requests in this period.")}
                series={[
                  {
                    id: metric,
                    label: t("kaeso.share.thisPeriod", "This period"),
                    color: m.color,
                    points: days.map((d) => ({ t: d, v: m.of(point(rows, d)) })),
                  },
                  {
                    id: `${metric}-before`,
                    label: t("kaeso.share.before", "The days before"),
                    color: PREVIOUS,
                    dashed: true,
                    points: days.map((d) => ({ t: d, v: m.of(point(before, new Date(Date.parse(`${d}T00:00:00Z`) - shift).toISOString().slice(0, 10))) })),
                  },
                ]}
              />
            </div>
          </CellInset>
        </DashboardCell>
      </Dashboard>
      {breakdowns.length > 0 && (
        <Dashboard aria-label={t("kaeso.share.breakdowns", "Where the usage went")} columns={2}>
          {breakdowns.map((b, i) => (
            <Breakdown key={b.by} b={b} metric={metric} money={money} wide={rowAlone(i, breakdowns.length)} />
          ))}
        </Dashboard>
      )}
    </>
  );
}

// The last of an odd number of cells takes the whole row, or its empty neighbour shows the grid's hairline colour.
const rowAlone = (i: number, count: number) => count % 2 === 1 && i === count - 1;

function Breakdown({ b, metric, money, wide }: { b: SharedBreakdown; metric: MetricId; money: Money; wide: boolean }) {
  const m = METRICS[metric];
  const ranked = b.rows
    .map((r) => ({ r, value: m.rank(asUsage(r, money.code)) }))
    .sort((a, c) => c.value - a.value)
    .slice(0, 10);
  const top = ranked[0]?.value ?? 0;
  return (
    <DashboardCell className={wide ? "lg:col-span-2" : "lg:col-span-1"} title={BREAKDOWN_TITLE[b.by]?.() ?? b.by}>
      <RankList
        aria-label={BREAKDOWN_TITLE[b.by]?.() ?? b.by}
        empty={t("kaeso.share.nothing", "Nothing in this period")}
        items={ranked.map(({ r, value }, i) => ({
          id: `${b.by}-${i}`,
          label: rowLabel(b.by, r),
          value: m.formatRank(value, money),
          share: top > 0 ? value / top : 0,
        }))}
      />
    </DashboardCell>
  );
}

const CENTS = { decimals: 2 };

/** Where a budget is heading at its pace so far: its use scaled to the whole period. */
export function forecast(l: SharedLimit, now: number): number | undefined {
  if (!l.used || !l.resets_at || (l.period !== "day" && l.period !== "month")) return undefined;
  const end = Date.parse(l.resets_at);
  const start = new Date(end);
  if (l.period === "day") start.setUTCDate(start.getUTCDate() - 1);
  else start.setUTCMonth(start.getUTCMonth() - 1);
  const elapsed = (now - start.getTime()) / (end - start.getTime());
  return elapsed > 0 && elapsed <= 1 ? Number(l.used) / elapsed : undefined;
}

function limitName(l: SharedLimit): string {
  if (l.level === "project") return t("kaeso.share.project", "The project");
  if (l.name) return l.name;
  return l.level === "key"
    ? t("kaeso.share.keyNumber", "Key {n}", { n: Number(l.number) })
    : t("kaeso.share.memberNumber", "Member {n}", { n: Number(l.number) });
}

function Budgets({ v }: { v: SharedView }) {
  const now = useMinute();
  if (v.limits.length === 0) {
    return <Text tone="secondary">{t("kaeso.share.noLimits", "The project has no limits or budgets that this link shows.")}</Text>;
  }
  return (
    <Dashboard aria-label={t("kaeso.share.limits", "Limits and budgets")} columns={2} headingLevel={2}>
      {v.limits.map((l, i) => {
        const amount = Number(l.amount);
        const ahead = forecast(l, now);
        const use = limitUse(l, (values) => t("kaeso.share.usedOf", "{used} of {amount}", values), CENTS);
        return (
          <DashboardCell
            key={i}
            className={rowAlone(i, v.limits.length) ? "lg:col-span-2" : "lg:col-span-1"}
            title={`${limitName(l)} · ${limitKindLabel(l.kind)}`}
          >
            <CellInset>
              <Meter
                value={Number(l.used) / amount}
                label={[periodLabel(l.period), l.model].filter(Boolean).join(" · ")}
                valueLabel={<LimitFigure figure={use} />}
                valueText={use.exact}
                caption={
                  ahead !== undefined
                    ? t("kaeso.share.forecast", "Heading for {amount} at this pace", {
                        amount: limitAmount(l.kind, ahead.toFixed(6), { ...CENTS, currency: l.currency }).short,
                      })
                    : l.hard
                      ? t("kaeso.share.hard", "Requests stop once it's used up")
                      : t("kaeso.share.soft", "Only alerts once it's used up")
                }
              />
            </CellInset>
          </DashboardCell>
        );
      })}
    </Dashboard>
  );
}
