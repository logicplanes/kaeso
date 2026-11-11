import { useEffect, useState, useSyncExternalStore } from "react";
import { Banner } from "@logicplanes/kaeso-ui/banner";
import { Button } from "@logicplanes/kaeso-ui/button";
import { Code } from "@logicplanes/kaeso-ui/code";
import { BarChart, TimeseriesChart } from "@logicplanes/kaeso-ui/charts";
import { RankList, type RankItem } from "@logicplanes/kaeso-ui/dashboard";
import { Skeleton } from "@logicplanes/kaeso-ui/skeleton";
import {
  formatDateRange,
  formatDuration,
  formatNumber,
  formatPercent,
  formatTimeZone,
  formatTokens,
  formatUSD,
  setFormatTimeZone,
} from "@logicplanes/kaeso-ui/format";
import { Heading } from "@logicplanes/kaeso-ui/heading";
import { LocaleProvider, loadUiMessages, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import { icons } from "@logicplanes/kaeso-ui/icon";
import { Logo } from "@logicplanes/kaeso-ui/logo";
import { Segmented } from "@logicplanes/kaeso-ui/segmented";
import { Text } from "@logicplanes/kaeso-ui/text";
import { ThemeProvider } from "@logicplanes/kaeso-ui/theme";
import { TooltipProvider } from "@logicplanes/kaeso-ui/tooltip";
import { GROUPS, METRICS, numeric, RANGES, type Group, type Metric, type Range, type Stats, type Value } from "./data";
import { setLocale, t } from "./i18n";
import type { State, Store } from "./store";

const metricLabel = (m: Metric) =>
  ({
    requests: t("stats.metric.requests", "Requests"),
    tokens: t("stats.metric.tokens", "Tokens"),
    cost: t("stats.metric.cost", "Cost"),
    latency: t("stats.metric.latency", "Latency"),
    errors: t("stats.metric.errors", "Errors"),
  })[m];

const rangeLabel = (r: Range) => ({ "24h": t("stats.range.day", "24h"), "7d": t("stats.range.week", "7d"), "30d": t("stats.range.month", "30d") })[r];

const rangeWords = (r: Range) =>
  ({ "24h": t("stats.period.day", "Last 24 hours"), "7d": t("stats.period.week", "Last 7 days"), "30d": t("stats.period.month", "Last 30 days") })[r];

const groupLabel = (g: Group) =>
  ({ model: t("stats.group.model", "Model"), provider: t("stats.group.provider", "Provider"), key: t("stats.group.key", "Key") })[g];

export function money(amount: string): string {
  const n = Number(amount);
  if (n > 0 && n < 0.005) return t("stats.lessThan", "<{amount}", { amount: formatUSD("0.01", { decimals: 2 }) });
  return formatUSD(amount, { decimals: 2 });
}

export function formatValue(metric: Metric, v: Value): string {
  if (v === null) return "—";
  if (metric === "cost") return money(String(v));
  const n = numeric(v) ?? 0;
  if (metric === "latency") return formatDuration(n);
  if (metric === "tokens") return formatTokens(n);
  return formatNumber(n);
}

const chartValue = (metric: Metric) => (v: number) => (metric === "cost" ? formatUSD(String(v), { decimals: 2 }) : formatValue(metric, v));

function groupName(row: { key: string; label: string }): string {
  if (row.key === "playground") return t("stats.playground", "Playground");
  if (row.key === "") return t("stats.unknown", "Unknown");
  return row.label;
}

function Chart({ stats, loading }: { stats: Stats; loading: boolean }) {
  const label = t("stats.chartLabel", "{metric} over time", { metric: metricLabel(stats.metric) });
  const format = chartValue(stats.metric);
  const empty = stats.series.every((p) => !numeric(p.value));
  if (stats.step === "day") {
    const days = stats.series.map((p) => p.start);
    return (
      <BarChart
        aria-label={label}
        categories={days.map((d) => formatDateRange(d, d))}
        dates={days}
        series={[{ id: stats.metric, label: metricLabel(stats.metric), values: stats.series.map((p) => numeric(p.value) ?? 0) }]}
        formatValue={format}
        partial={{ last: true, ongoing: true }}
        loading={loading}
        empty={empty}
        emptyText={t("stats.emptyChart", "Nothing in this period.")}
        height={200}
      />
    );
  }
  const points = stats.series.flatMap((p) => {
    const v = numeric(p.value);
    return v === null && stats.metric === "latency" ? [] : [{ t: new Date(p.start), v: v ?? 0 }];
  });
  return (
    <TimeseriesChart
      aria-label={label}
      kind="area"
      hideLegend
      series={[{ id: stats.metric, label: metricLabel(stats.metric), points }]}
      formatValue={format}
      partial={{ last: true, ongoing: true }}
      loading={loading}
      empty={empty}
      emptyText={t("stats.emptyChart", "Nothing in this period.")}
      height={200}
    />
  );
}

function Totals({ stats }: { stats: Stats | null }) {
  const totals = stats?.totals;
  const loading = !totals;
  const errorShare = totals && totals.requests > 0 ? formatPercent(totals.errors / totals.requests) : null;
  const items = [
    { id: "requests", label: metricLabel("requests"), value: totals ? formatNumber(totals.requests) : "", detail: undefined },
    { id: "tokens", label: metricLabel("tokens"), value: totals ? formatTokens(totals.tokens) : "", detail: undefined },
    {
      id: "cost",
      label: metricLabel("cost"),
      value: totals ? formatValue("cost", totals.cost) : "",
      detail: totals && totals.cost === null ? t("stats.costHidden", "Hidden from you") : undefined,
    },
    {
      id: "latency",
      label: t("stats.latencyP95", "Latency, p95"),
      value: totals ? formatValue("latency", totals.latency_p95_ms) : "",
      detail: totals?.latency_p50_ms != null ? t("stats.latencyP50", "p50 {value}", { value: formatDuration(totals.latency_p50_ms) }) : undefined,
    },
    {
      id: "errors",
      label: metricLabel("errors"),
      value: totals ? formatNumber(totals.errors) : "",
      detail: errorShare ? t("stats.errorShare", "{share} of requests", { share: errorShare }) : undefined,
    },
  ];
  return (
    <div role="group" aria-label={t("stats.totals", "Totals")} className="mx-24 grid grid-cols-2 gap-x-24 gap-y-16 sm:grid-cols-3 md:grid-cols-5">
      {items.map((item) => (
        <div key={item.id} className="flex min-w-0 flex-col gap-2">
          <Text variant="small" tone="secondary" truncate>
            {item.label}
          </Text>
          <Text variant="title" weight="semibold" tabular truncate>
            {loading ? <Skeleton shape="line" className="w-64" /> : item.value}
          </Text>
          <Text variant="small" tone="tertiary" truncate>
            {item.detail ?? "\u00a0"}
          </Text>
        </div>
      ))}
    </div>
  );
}

function Breakdown({ stats, loading }: { stats: Stats | null; loading: boolean }) {
  const metric = stats?.metric ?? "requests";
  const rows = stats?.groups ?? [];
  const top = Math.max(0, ...rows.map((g) => numeric(g.value) ?? 0));
  const total = rows.reduce((a, g) => a + (numeric(g.value) ?? 0), 0);
  const items: RankItem[] = rows.map((g) => {
    const v = numeric(g.value);
    return {
      id: g.key,
      label: <span className="min-w-0 truncate">{groupName(g)}</span>,
      value: formatValue(metric, g.value),
      secondary:
        metric === "latency"
          ? t("stats.ofRequests", "{requests} requests", { requests: formatNumber(g.requests) })
          : v !== null && total > 0
            ? formatPercent(v / total)
            : undefined,
      ...(v !== null && top > 0 ? { share: v / top } : {}),
    };
  });
  const group = stats?.group_by ?? "model";
  return (
    <RankList
      aria-label={t("stats.breakdown", "Breakdown by {group}", { group: groupLabel(group) })}
      items={items}
      loading={loading}
      rows={Math.max(3, Math.min(rows.length, 8))}
      empty={t("stats.emptyBreakdown", "Nothing in this period.")}
      className="mx-24"
    />
  );
}

// Kaeso's messages name fields and rights as code spans; each part stays text.
export function Message({ text }: { text: string }) {
  return <>{text.split("`").map((part, i) => (i % 2 ? <Code key={i}>{part}</Code> : part))}</>;
}

function Problem({ state, store }: { state: State; store: Store }) {
  const failure = state.failure;
  if (!failure) return null;
  if (failure.kind === "cancelled") return <Banner>{t("stats.cancelled", "The call was cancelled.")}</Banner>;
  if (failure.kind === "rights") {
    const url = failure.approveUrl;
    return (
      <Banner
        tone="warning"
        title={t("stats.rightsTitle", "Kaeso needs your approval")}
        actions={
          url && state.capabilities.openLinks ? (
            <Button size="sm" variant="secondary" trailingIcon={icons.openExternal} onClick={() => void store.open(url)}>
              {t("stats.approve", "Approve in Kaeso")}
            </Button>
          ) : undefined
        }
      >
        <Message text={failure.message} />
      </Banner>
    );
  }
  return (
    <Banner tone="danger">
      <Message text={failure.message || t("stats.failed", "Kaeso couldn't load the usage; try again.")} />
    </Banner>
  );
}

function Header({ state, store }: { state: State; store: Store }) {
  const { context, capabilities, stats } = state;
  const canExpand = (context.availableDisplayModes ?? []).includes("fullscreen") && context.displayMode !== "fullscreen";
  return (
    <header className="mx-24 mt-20 flex items-center justify-between gap-12">
      <Logo.Wordmark size="nav" />
      <div className="flex items-center gap-8">
        {canExpand && (
          <Button size="sm" variant="ghost" icon={icons.sidePanel} aria-label={t("stats.expand", "Open beside the chat")} onClick={() => void store.expand()} />
        )}
        {stats && capabilities.openLinks && (
          <Button size="sm" variant="secondary" trailingIcon={icons.openExternal} onClick={() => void store.open(stats.console_url)}>
            {t("stats.open", "Open in Kaeso")}
          </Button>
        )}
      </div>
    </header>
  );
}

function Title({ stats, range }: { stats: Stats | null; range: Range }) {
  const scope = stats
    ? stats.scope.project
      ? t("stats.scopeProject", "{project} in {org}", { project: stats.scope.project.name, org: stats.scope.org.name })
      : stats.scope.org.name
    : "";
  const period = stats ? `${rangeWords(range)} · ${formatDateRange(stats.from, stats.to)}` : rangeWords(range);
  return (
    <div className="mx-24 flex min-w-0 flex-col gap-4">
      <Heading level={1} size="title">
        {t("stats.title", "Usage")}
      </Heading>
      <Text variant="small" tone="secondary" truncate>
        {scope ? `${scope} · ${period}` : period}
      </Text>
    </div>
  );
}

function Controls({ state, store }: { state: State; store: Store }) {
  const metric = state.ask.metric ?? "requests";
  const range = state.ask.range ?? "7d";
  const group = state.ask.group_by ?? "model";
  const disabled = !state.connected;
  return (
    <div className="mx-24 flex flex-wrap items-center gap-8">
      <Segmented
        size="sm"
        aria-label={t("stats.metricLabel", "Metric")}
        options={METRICS.map((m) => ({ value: m, label: metricLabel(m) }))}
        value={metric}
        disabled={disabled}
        onValueChange={(v) => void store.change({ metric: v as Metric })}
      />
      <Segmented
        size="sm"
        aria-label={t("stats.rangeLabel", "Period")}
        options={RANGES.map((r) => ({ value: r, label: rangeLabel(r) }))}
        value={range}
        disabled={disabled}
        onValueChange={(v) => void store.change({ range: v as Range })}
      />
      <Segmented
        size="sm"
        aria-label={t("stats.groupLabel", "Break down by")}
        options={GROUPS.map((g) => ({ value: g, label: groupLabel(g) }))}
        value={group}
        disabled={disabled}
        onValueChange={(v) => void store.change({ group_by: v as Group })}
      />
    </div>
  );
}

function useLibraryMessages(locale: string): UiMessages | undefined {
  const [messages, setMessages] = useState<{ locale: string; messages: UiMessages } | null>(null);
  useEffect(() => {
    let live = true;
    void loadUiMessages(locale).then((m) => live && setMessages({ locale, messages: m }));
    return () => {
      live = false;
    };
  }, [locale]);
  return messages?.locale === locale ? messages.messages : undefined;
}

function insets(state: State) {
  const root = document.documentElement;
  const s = state.context.safeAreaInsets;
  for (const side of ["top", "right", "bottom", "left"] as const) root.style.setProperty(`--safe-${side}`, `${s?.[side] ?? 0}px`);
  root.toggleAttribute("data-fullscreen", state.context.displayMode === "fullscreen");
}

export function StatsView({ store }: { store: Store }) {
  const state = useSyncExternalStore(store.subscribe, store.get);
  const locale = setLocale(state.context.locale);
  const messages = useLibraryMessages(locale);
  if (state.context.timeZone && formatTimeZone() !== state.context.timeZone) setFormatTimeZone(state.context.timeZone);
  useEffect(() => insets(state), [state]);
  const range = state.ask.range ?? "7d";
  const stats = state.stats;
  return (
    <LocaleProvider locale={locale} messages={messages}>
      <ThemeProvider defaultTheme={state.context.theme ?? "system"}>
        <TooltipProvider>
          <main lang={locale} className="flex flex-col gap-20 mb-24">
            <Header state={state} store={store} />
            <Title stats={stats} range={range} />
            {state.failure && (
              <div className="mx-24">
                <Problem state={state} store={store} />
              </div>
            )}
            {(stats || !state.failure) && (
              <>
                <Totals stats={stats} />
                <Controls state={state} store={store} />
                <div className="mx-24">
                  {stats ? (
                    <Chart stats={stats} loading={state.loading} />
                  ) : (
                    <BarChart aria-label={t("stats.loading", "Loading usage")} categories={[]} series={[]} loading height={200} />
                  )}
                </div>
                <Breakdown stats={stats} loading={state.loading} />
              </>
            )}
          </main>
        </TooltipProvider>
      </ThemeProvider>
    </LocaleProvider>
  );
}
