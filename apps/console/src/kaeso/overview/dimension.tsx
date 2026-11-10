import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Avatar,
  CellInset,
  DataTable,
  Dashboard,
  DashboardCell,
  Delta,
  Empty,
  ModelLabel,
  RankList,
  Readout,
  ShareBar,
  Text,
  col,
  type RankItem,
} from "@logicplanes/kaeso-ui";
import { formatNumber, formatPercent, formatTokens } from "@logicplanes/kaeso-ui/format";
import type { UsageRow } from "~/api/generated/types";
import { orgUsageQuery, projectUsageQuery } from "~/api/queries/usage";
import { previousPeriod } from "~/format/period";
import { t } from "~/i18n";
import { totalTokens } from "~/kaeso/tokens";
import { costUnknown, costWidth, hasMedia, mediaSummary, UsageCost } from "~/kaeso/media-usage";
import { periodUSD, tileUSD } from "~/kaeso/usage-panels";
import { projectPath } from "~/shell/pages";
import { usePhone } from "~/shell/use-phone";
import type { Overview } from "./data";
import { cacheHit, change, type Dimension } from "./metrics";

interface Entity {
  key: string;
  name: string;
  requests: number;
  errors: number;
  tokens: number;
  cost: number;
  cache: number | null;
  row: UsageRow;
  known: boolean;
  before: number | undefined;
  delta: number;
  change: number | undefined;
}

const MOVERS = 5;
const SHARES = 5;

export function dimensionTitle(dim: Dimension): string {
  switch (dim) {
    case "model":
      return t("kaeso.analytics.models", "Models");
    case "key":
      return t("kaeso.analytics.keys", "Keys");
    case "member":
      return t("kaeso.analytics.members", "Members");
    case "project":
      return t("kaeso.analytics.projects", "Projects");
  }
}

export function DimensionView({ o, dim }: { o: Overview; dim: Dimension }) {
  const navigate = useNavigate();
  const before = previousPeriod(o.from, o.to);
  const usage = (from: string, to: string) => (o.project ? projectUsageQuery(o.org, o.project, from, to, dim) : orgUsageQuery(o.org, from, to, dim));
  const now = useQuery({ ...usage(o.from, o.to), enabled: o.canUsage, placeholderData: (p) => p });
  const prev = useQuery({ ...usage(before.from, before.to), enabled: o.canUsage && !!o.compare });
  const was = new Map((prev.data ?? []).map((r) => [r.key, r.requests]));
  const nameOf = (key: string) =>
    dim === "model" ? o.modelName(key) : dim === "key" ? o.keyName(key) : dim === "member" ? o.memberName(key) : (o.projectOf(key)?.name ?? key);
  const phone = usePhone();
  const rows = now.data ?? [];
  const entities: Entity[] = rows
    .map((r: UsageRow) => {
      const b = prev.data ? (was.get(r.key) ?? 0) : undefined;
      return {
        key: r.key,
        name: nameOf(r.key),
        requests: r.requests,
        errors: r.errors,
        tokens: totalTokens(r),
        cost: Number(r.cost ?? 0),
        cache: cacheHit([r]),
        row: r,
        known: !costUnknown(r),
        before: b,
        delta: r.requests - (b ?? 0),
        change: b ? change(r.requests, b) : undefined,
      };
    })
    .sort((a, b) => b.requests - a.requests);
  const requests = entities.reduce((a, e) => a + e.requests, 0);
  const total = requests || 1;
  const totalCost = entities.reduce((a, e) => a + e.cost, 0);
  const unpriced = entities.reduce((a, e) => a + e.row.unpriced, 0);
  const unpricedNote = unpriced
    ? t(
        "kaeso.analytics.costShareUnpriced",
        "{count, plural, one {# request without a price isn't included.} other {# requests without a price aren't included.}}",
        { count: unpriced },
      )
    : undefined;

  const label = (e: Entity) => {
    if (dim === "model") {
      const [provider = "", ...rest] = e.key.split("/");
      const model = rest.join("/");
      if (!model) return <Text tone="secondary">{e.name}</Text>;
      return (
        <ModelLabel
          className="min-w-0"
          provider={provider}
          providerName={o.providers.name(provider)}
          model={model}
          {...(e.name !== model ? { name: e.name } : {})}
        />
      );
    }
    if (dim === "project") {
      const p = o.projectOf(e.key);
      return (
        <span className="flex min-w-0 items-center gap-8">
          <Avatar name={e.name} {...(p?.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
          <span className="min-w-0 truncate">{e.name}</span>
        </span>
      );
    }
    return <span className="min-w-0 truncate">{e.name}</span>;
  };
  const logs = o.project && o.showErrors ? `${projectPath(o.org, o.project)}/logs` : undefined;
  const filter = (key: string) => (dim === "model" ? `model=${encodeURIComponent(key.split("/").slice(1).join("/"))}` : `${dim}=${encodeURIComponent(key)}`);
  const open =
    dim === "project"
      ? (key: string) => void navigate({ href: projectPath(o.org, key) })
      : logs
        ? (key: string) => void navigate({ href: `${logs}?${filter(key)}` })
        : undefined;

  const shareSlices = (value: (e: Entity) => number) => {
    const ranked = [...entities].filter((e) => value(e) > 0).sort((a, b) => value(b) - value(a));
    const top = ranked.slice(0, SHARES).map((e) => ({ id: e.key, label: e.name, value: value(e) }));
    const rest = ranked.slice(SHARES).reduce((a, e) => a + value(e), 0);
    return rest > 0 ? [...top, { id: "other", label: t("kaeso.overview.other", "Other"), value: rest, color: "var(--k-fill-3)" }] : top;
  };
  const moved = entities.filter((e) => e.before !== undefined && e.delta !== 0);
  const mover = (e: Entity): RankItem => ({
    id: e.key,
    label: label(e),
    value: `${e.delta > 0 ? "+" : "−"}${formatNumber(Math.abs(e.delta))}`,
    secondary: e.before ? `${e.delta > 0 ? "↑" : "↓"}${formatPercent(Math.abs(e.delta) / e.before, 0)}` : t("kaeso.analytics.newInPeriod", "New"),
    share: Math.abs(e.delta),
  });
  const byDelta = [...moved].sort((a, b) => b.delta - a.delta);
  const growing = byDelta
    .filter((e) => e.delta > 0)
    .slice(0, MOVERS)
    .map(mover);
  const shrinking = byDelta
    .filter((e) => e.delta < 0)
    .reverse()
    .slice(0, MOVERS)
    .map(mover);
  const busiest = entities[0];
  const priciest = [...entities].filter((e) => e.cost > 0).sort((a, b) => b.cost - a.cost)[0];
  const flakiest = [...entities].filter((e) => e.requests >= 10).sort((a, b) => b.errors / b.requests - a.errors / a.requests)[0];
  const loading = now.isPending;

  const columns = [
    col.custom(
      { id: "name", header: dimensionTitle(dim), size: 220, minWidth: 140, priority: 1, grow: true },
      (e: Entity) => e.name,
      (_v, e) => label(e),
    ),
    col.number({ id: "requests", header: t("common.requests", "Requests"), size: 96, priority: 1 }, (e: Entity) => e.requests),
    ...(o.compare
      ? [
          col.custom(
            { id: "change", header: t("kaeso.analytics.change", "Change"), size: 96, priority: 2 },
            (e: Entity) => e.change ?? Number.NEGATIVE_INFINITY,
            (_v, e) =>
              e.change === undefined ? (
                <Text variant="small" tone="tertiary">
                  {e.before === 0 ? t("kaeso.analytics.newInPeriod", "New") : "—"}
                </Text>
              ) : (
                <Delta value={e.change} better="none" />
              ),
            { align: "end" },
          ),
        ]
      : []),
    col.custom(
      { id: "share", header: t("kaeso.analytics.share", "Share"), size: 84, priority: 3 },
      (e: Entity) => e.requests / total,
      (v: number) => formatPercent(v, 0),
      { align: "end" },
    ),
    col.custom(
      {
        id: "rate",
        header: t("kaeso.overview.errorRate", "Error rate"),
        size: 104,
        priority: 2,
        tone: (e: Entity) => (e.requests && e.errors / e.requests >= 0.05 ? "danger" : undefined),
      },
      (e: Entity) => (e.requests ? e.errors / e.requests : 0),
      (v: number) => formatPercent(v),
      { align: "end" },
    ),
    col.tokens({ id: "tokens", header: t("common.tokens", "Tokens"), size: 96, priority: 3 }, (e: Entity) => e.tokens),
    col.custom(
      { id: "per", header: t("kaeso.analytics.avgTokens", "Avg tokens"), size: 112, priority: 4 },
      (e: Entity) => (e.requests ? e.tokens / e.requests : 0),
      (v: number) => formatTokens(Math.round(v)),
      { align: "end" },
    ),
    col.custom(
      { id: "cache", header: t("kaeso.overview.cacheHit", "Cache hit"), size: 104, priority: 4 },
      (e: Entity) => e.cache ?? -1,
      (v: number) => (v < 0 ? "—" : formatPercent(v, 0)),
      { align: "end" },
    ),
    ...(o.showCost
      ? [
          col.custom(
            { id: "cost", header: t("common.cost", "Cost"), size: costWidth(rows, 96, phone), priority: 1 },
            (e: Entity) => (e.known ? e.cost : -1),
            (_v: number, e: Entity) => <UsageCost row={e.row} />,
            { align: "end" },
          ),
          col.custom(
            { id: "unit", header: t("kaeso.analytics.avgCost", "Avg cost"), size: 112, priority: 5 },
            (e: Entity) => (e.known && e.requests > e.row.unpriced ? e.cost / (e.requests - e.row.unpriced) : -1),
            (v: number) => (v < 0 ? <Text tone="secondary">{t("kaeso.usage.costUnknown", "Unknown")}</Text> : tileUSD(v.toFixed(8))),
            { align: "end" },
          ),
        ]
      : []),
    ...(hasMedia(rows)
      ? [
          col.custom(
            { id: "media", header: t("kaeso.usage.media.header", "Media"), size: 180, minWidth: 120, priority: 3 },
            (e: Entity) => mediaSummary(e.row.media),
            (v: string) => (v ? <Text truncate>{v}</Text> : <Text tone="secondary">—</Text>),
          ),
        ]
      : []),
  ];

  return (
    <div className="flex flex-col">
      <Dashboard aria-label={dimensionTitle(dim)} columns={2} wideColumns={4} busy={now.isPlaceholderData} headingLevel={2}>
        <DashboardCell className="lg:col-span-1" title={t("kaeso.analytics.active", "Active in the period")}>
          <Readout
            size="sm"
            value={formatNumber(entities.length)}
            detail={t("kaeso.analytics.ofRequests", "{requests} requests", { requests: formatNumber(requests) })}
            loading={loading}
          />
        </DashboardCell>
        <DashboardCell className="lg:col-span-1" title={t("kaeso.analytics.busiest", "Most requests")}>
          <Readout
            size="sm"
            value={busiest ? busiest.name : "—"}
            {...(busiest ? { detail: t("kaeso.analytics.shareOf", "{share} of requests", { share: formatPercent(busiest.requests / total, 0) }) } : {})}
            loading={loading}
          />
        </DashboardCell>
        <DashboardCell
          className="lg:col-span-1"
          title={o.showCost ? t("kaeso.analytics.priciest", "Highest cost") : t("kaeso.analytics.mostTokens", "Most tokens")}
        >
          {o.showCost ? (
            <Readout
              size="sm"
              value={priciest ? priciest.name : unpriced ? t("kaeso.usage.costUnknown", "Unknown") : "—"}
              {...(priciest && totalCost
                ? {
                    detail: unpriced
                      ? t("kaeso.analytics.costShareKnown", "{amount}, {share} of the known cost", {
                          amount: periodUSD(priciest.cost.toFixed(6)),
                          share: formatPercent(priciest.cost / totalCost, 0),
                        })
                      : t("kaeso.analytics.costShare", "{amount}, {share} of cost", {
                          amount: periodUSD(priciest.cost.toFixed(6)),
                          share: formatPercent(priciest.cost / totalCost, 0),
                        }),
                  }
                : {})}
              loading={loading}
            />
          ) : (
            <Readout size="sm" value={[...entities].sort((a, b) => b.tokens - a.tokens)[0]?.name ?? "—"} loading={loading} />
          )}
        </DashboardCell>
        <DashboardCell className="lg:col-span-1" title={t("kaeso.analytics.flakiest", "Highest error rate")}>
          <Readout
            size="sm"
            value={flakiest && flakiest.errors ? flakiest.name : "—"}
            {...(flakiest && flakiest.errors
              ? {
                  detail: t("kaeso.analytics.failedOf", "{rate} of {requests} failed", {
                    rate: formatPercent(flakiest.errors / flakiest.requests),
                    requests: formatNumber(flakiest.requests),
                  }),
                }
              : {})}
            loading={loading}
          />
        </DashboardCell>
        <DashboardCell
          className={o.showCost ? "lg:col-span-1 xl:col-span-2" : "lg:col-span-2 xl:col-span-4"}
          title={t("kaeso.analytics.requestShare", "Share of requests")}
        >
          <CellInset>
            <ShareBar
              aria-label={t("kaeso.analytics.requestShare", "Share of requests")}
              slices={shareSlices((e) => e.requests)}
              formatValue={(v) => formatNumber(v)}
            />
          </CellInset>
        </DashboardCell>
        {o.showCost && (
          <DashboardCell className="lg:col-span-1 xl:col-span-2" title={t("kaeso.analytics.costShareTitle", "Share of cost")}>
            <CellInset>
              <ShareBar
                aria-label={t("kaeso.analytics.costShareTitle", "Share of cost")}
                slices={shareSlices((e) => e.cost)}
                formatValue={(v) => periodUSD(v.toFixed(6))}
              />
              {unpricedNote && (
                <Text as="p" variant="caption" tone="tertiary" className="mt-12">
                  {unpricedNote}
                </Text>
              )}
            </CellInset>
          </DashboardCell>
        )}
        {o.compare && (
          <>
            <DashboardCell className="lg:col-span-1 xl:col-span-2" title={t("kaeso.analytics.growing", "Grew the most, {compare}", { compare: o.compare })}>
              <RankList
                aria-label={t("kaeso.analytics.growingLabel", "Grew the most")}
                items={growing}
                loading={loading || prev.isPending}
                empty={t("kaeso.analytics.noneGrew", "Nothing grew")}
                {...(open ? { onOpen: open } : {})}
              />
            </DashboardCell>
            <DashboardCell className="lg:col-span-1 xl:col-span-2" title={t("kaeso.analytics.shrinking", "Shrank the most, {compare}", { compare: o.compare })}>
              <RankList
                aria-label={t("kaeso.analytics.shrinkingLabel", "Shrank the most")}
                items={shrinking}
                loading={loading || prev.isPending}
                empty={t("kaeso.analytics.noneShrank", "Nothing shrank")}
                {...(open ? { onOpen: open } : {})}
              />
            </DashboardCell>
          </>
        )}
      </Dashboard>
      <div className="mt-48">
        <DataTable
          aria-label={t("kaeso.analytics.tableLabel", "{dimension} in the period", { dimension: dimensionTitle(dim) })}
          data={entities}
          columns={columns}
          getRowId={(e) => e.key}
          loading={loading}
          error={now.isError}
          onRetry={() => void now.refetch()}
          {...(open ? { onRowClick: (e: Entity) => open(e.key) } : {})}
          empty={<Empty size="compact" title={t("project.overview.noRequests", "No requests in this period")} />}
        />
      </div>
    </div>
  );
}
