import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Badge,
  Button,
  DataTable,
  DateRangePicker,
  DisplayOptions,
  Empty,
  FilterChip,
  type FilterChipChoice,
  InlineCopyText,
  Menu,
  Pill,
  ProviderBadge,
  SearchInput,
  Select,
  StatusCode,
  Text,
  Toolbar,
  Tooltip,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import { isApiError } from "~/api/errors";
import type { LogView } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { logsInfiniteQuery, newestLogsQuery, type LogFilters, type LogRange } from "~/api/queries/logs";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { ERROR_KIND_IDS, PROVIDER_IDS, errorKindLabel, providerLabel } from "~/format/labels";
import { requestUSD } from "~/format/money";
import { utcDay } from "~/format/period";
import { pickedDay, pickerDate } from "~/format/picked-day";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyName } from "../keys/party";
import { ExportDialog } from "./export-dialog";
import { LogInspector } from "./log-inspector";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { locale, t, type Locale } from "~/i18n";
import { common } from "~/i18n/common";
import { dateLocale } from "./timing";
import { readStoredJson, writeStored } from "~/storage";

export type LogsSearch = LogFilters & { log?: string; live?: boolean };

const ranges = (): { value: LogRange; label: string }[] => [
  { value: "1h", label: t("logs.range.hour", "Last hour") },
  { value: "24h", label: t("logs.range.day", "Last 24 hours") },
  { value: "7d", label: t("logs.range.week", "Last 7 days") },
  { value: "30d", label: t("logs.range.month", "Last 30 days") },
];
const STATUSES = [200, 400, 401, 403, 404, 408, 413, 422, 429, 499, 500, 502, 503, 504, 529];
type FilterId = "key" | "member" | "model" | "provider" | "status" | "error" | "stream" | "source" | "failed";
type ChoiceFilterId = Exclude<FilterId, "failed">;
const FILTER_IDS: FilterId[] = ["key", "member", "model", "provider", "status", "failed", "error", "stream", "source"];
const filterLabel = (id: FilterId): string =>
  ({
    key: t("logs.filter.key", "Key"),
    member: t("logs.filter.member", "Member"),
    model: common.model(),
    provider: common.provider(),
    status: common.status(),
    failed: t("logs.filter.failed", "Failed only"),
    error: t("logs.filter.error", "Error kind"),
    stream: t("logs.filter.streamed", "Streamed"),
    source: t("logs.filter.source", "Source"),
  })[id];
const COLUMNS = [
  "time",
  "status",
  "endpoint",
  "model",
  "provider",
  "stream",
  "duration",
  "ttfb",
  "tokens",
  "cost",
  "key",
  "principal",
  "request",
  "agent",
] as const;
type ColumnId = (typeof COLUMNS)[number];
const columnLabel = (id: ColumnId): string =>
  ({
    time: t("logs.column.started", "Started"),
    status: common.status(),
    endpoint: t("logs.column.endpoint", "Endpoint"),
    model: common.model(),
    provider: common.provider(),
    stream: t("logs.column.stream", "Stream"),
    duration: t("logs.column.duration", "Duration"),
    ttfb: t("logs.column.firstByte", "First byte"),
    tokens: common.tokens(),
    cost: common.cost(),
    key: t("logs.column.key", "Key"),
    principal: t("logs.column.principal", "Principal"),
    request: t("logs.column.requestId", "Request id"),
    agent: t("logs.column.userAgent", "User agent"),
  })[id];
const DAY_TIME_WIDTH: Record<Locale, number> = { en: 160, de: 168, "zh-CN": 176 };
const DEFAULT_COLUMNS: ColumnId[] = COLUMNS.filter((c) => c !== "request" && c !== "agent");

function scrollParentOf(el: HTMLElement | null): HTMLElement | null {
  for (let node = el?.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") return node;
  }
  return document.scrollingElement as HTMLElement | null;
}

const COLUMNS_KEY = "kaeso:logs-columns";

function storedColumns(): ColumnId[] {
  const v = readStoredJson(COLUMNS_KEY);
  return Array.isArray(v) ? (v.filter((c) => (COLUMNS as readonly string[]).includes(c)) as ColumnId[]) : DEFAULT_COLUMNS;
}

export function LogsPage({ org, project, search }: { org: string; project: string; search: LogsSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const { log, live, ...filters } = search;
  const logs = useInfiniteQuery(logsInfiniteQuery(org, project, filters));
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  const catalogue = useQuery(catalogueQuery(org, project));
  const partyName = usePartyName(org, project);
  const [columns, setColumns] = useState<ColumnId[]>(storedColumns);
  const [density, setDensity] = useState<"default" | "compact">("default");
  const [adding, setAdding] = useState<FilterId | null>(null);
  const [exporting, setExporting] = useState(false);
  const scroller = useRef<HTMLDivElement | null>(null);

  const setSearch = (patch: Partial<Record<keyof LogsSearch, string | number | boolean | undefined>>) => {
    const next = Object.fromEntries(Object.entries({ ...search, ...patch }).filter(([, v]) => v !== undefined && v !== false && v !== "")) as LogsSearch;
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: next, replace: true });
  };

  const queryClient = useQueryClient();
  const filterKey = JSON.stringify(filters);
  const [liveState, setLiveState] = useState<{ key: string; extra: LogView[]; pending: LogView[] }>({ key: "", extra: [], pending: [] });
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const base = useMemo(() => logs.data?.pages.flatMap((p) => p.data) ?? [], [logs.data]);
  const liveKey = `${filterKey}|${live ? "live" : ""}`;
  const { extra, pending } = liveState.key === liveKey ? liveState : { extra: [], pending: [] };
  const shownIds = useEffectEvent(() => new Set([...extra, ...pending, ...base].map((r) => r.id)));
  useEffect(() => {
    if (!live) return;
    let polling = true;
    const tick = async () => {
      const newest = await queryClient.fetchQuery({ ...newestLogsQuery(org, project, JSON.parse(filterKey) as LogFilters), staleTime: 0 }).catch(() => []);
      if (!polling) return;
      const shown = shownIds();
      const incoming = newest.filter((r) => !shown.has(r.id));
      if (!incoming.length) return;
      const scrolled = !!scrollParentOf(scroller.current)?.scrollTop;
      setLiveState((s) => {
        const prev = s.key === liveKey ? s : { key: liveKey, extra: [], pending: [] };
        return scrolled ? { ...prev, pending: [...incoming, ...prev.pending] } : { ...prev, extra: [...incoming, ...prev.extra] };
      });
      if (!scrolled) setFresh(new Set(incoming.map((r) => r.id)));
    };
    const timer = setInterval(() => void tick(), 5000);
    return () => {
      polling = false;
      clearInterval(timer);
    };
  }, [live, org, project, filterKey, liveKey, queryClient]);
  const showPending = () => {
    setLiveState({ key: liveKey, extra: [...pending, ...extra], pending: [] });
    setFresh(new Set(pending.map((r) => r.id)));
    scrollParentOf(scroller.current)?.scrollTo({ top: 0 });
  };
  useEffect(() => {
    if (!fresh.size) return;
    const timer = setTimeout(() => setFresh(new Set()), 1000);
    return () => clearTimeout(timer);
  }, [fresh]);
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = logs;
  useEffect(() => {
    const more = scroller.current?.querySelector("nav");
    if (!more || !hasNextPage || isFetchingNextPage) return;
    const seen = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && void fetchNextPage(), {
      root: scrollParentOf(scroller.current),
      rootMargin: "800px 0px",
    });
    seen.observe(more);
    return () => seen.disconnect();
  });
  const rows = useMemo(() => {
    const seen = new Set<string>();
    return [...extra, ...base].filter((r) => (seen.has(r.id) ? false : (seen.add(r.id), true)));
  }, [extra, base]);

  useEffect(() => {
    if (!log) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || (e.target as HTMLElement).closest("input, textarea, [contenteditable], [role=combobox]")) return;
      if (e.key !== "j" && e.key !== "k") return;
      const i = rows.findIndex((r) => r.id === log);
      const next = rows[e.key === "j" ? i + 1 : i - 1];
      if (next) {
        e.preventDefault();
        setSearch({ log: next.id });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const keyName = (id: string) => keys.data?.find((k) => k.id === id)?.name ?? id;
  const spansDays = filters.from ? filters.from !== filters.to : filters.range !== "1h";
  const time = (iso: string) =>
    new Date(iso).toLocaleString(dateLocale(), {
      ...(spansDays ? { day: "numeric", month: "short" } : {}),
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    });

  const all = {
    time: col.custom(
      { id: "time", header: columnLabel("time"), size: spansDays ? DAY_TIME_WIDTH[locale()] : 112, priority: 1 },
      (r: LogView) => r.started_at,
      (v) => (
        <Text tabular>
          <time dateTime={v}>{time(v)}</time>
        </Text>
      ),
    ),
    status: col.custom(
      { id: "status", header: columnLabel("status"), size: 72, priority: 1 },
      (r: LogView) => r.status,
      (v) => <StatusCode code={v} />,
    ),
    endpoint: col.mono({ id: "endpoint", header: columnLabel("endpoint"), size: 152, priority: 3 }, (r: LogView) => r.endpoint),
    model: col.custom(
      { id: "model", header: columnLabel("model"), size: 280, minWidth: 104, priority: 1, grow: true },
      (r: LogView) => r.model,
      (_v, r) => <ModelCell row={r} />,
    ),
    provider: col.custom(
      { id: "provider", header: columnLabel("provider"), size: 112, priority: 3 },
      (r: LogView) => r.provider,
      (v) => <ProviderBadge provider={v} withName size="sm" />,
    ),
    stream: col.custom(
      { id: "stream", header: columnLabel("stream"), size: 104, priority: 4 },
      (r: LogView) => r.stream,
      (v) =>
        v ? (
          <Pill size="sm" variant="outline">
            {t("logs.streamPill", "Stream")}
          </Pill>
        ) : null,
    ),
    duration: col.duration({ id: "duration", header: columnLabel("duration"), size: 104, priority: 2 }, (r: LogView) => r.duration_ms),
    ttfb: col.custom(
      { id: "ttfb", header: columnLabel("ttfb"), size: 104, priority: 4 },
      (r: LogView) => r.ttfb_ms ?? null,
      (v) => (v === null ? "—" : formatDuration(v)),
      { align: "end" },
    ),
    tokens: col.custom(
      { id: "tokens", header: t("logs.column.tokensInOut", "Tokens in / out"), size: 136, priority: 2 },
      (r: LogView) => r.usage.input_tokens + r.usage.output_tokens,
      (_v, r) => <TokensCell row={r} />,
      { align: "end" },
    ),
    cost: col.custom(
      { id: "cost", header: columnLabel("cost"), size: 120, priority: 2 },
      (r: LogView) => r.cost,
      (v) => requestUSD(v),
      { align: "end" },
    ),
    key: col.custom(
      { id: "key", header: columnLabel("key"), size: 152, priority: 3 },
      (r: LogView) => r.key_id ?? null,
      (v) => (v ? keyName(v) : "—"),
    ),
    principal: col.custom(
      { id: "principal", header: columnLabel("principal"), size: 152, priority: 4 },
      (r: LogView) => r.principal_id,
      (_v, r) => partyName({ kind: r.principal_kind === "service_account" ? "service_account" : "user", id: r.principal_id }),
    ),
    request: col.custom(
      { id: "request", header: columnLabel("request"), size: 248, priority: 5 },
      (r: LogView) => r.id,
      (v) => <InlineCopyText value={v} label={t("logs.copyRequestId", "Copy request id")} />,
    ),
    agent: col.text({ id: "agent", header: columnLabel("agent"), size: 200, priority: 5 }, (r: LogView) => r.user_agent),
  };
  const shownColumns = COLUMNS.filter((c) => columns.includes(c)).map((c) => all[c]);

  const keyOptions = (keys.data ?? []).map((k) => ({ value: k.id, label: k.name }));
  const memberOptions = [
    ...(members.data ?? []).map((m) => ({ value: `user:${m.user.id}`, label: m.user.name })),
    ...(accounts.data ?? []).map((a) => ({ value: `sa:${a.id}`, label: t("logs.filter.serviceAccount", "{name} (service account)", { name: a.name }) })),
  ];
  const modelOptions = (catalogue.data ?? []).map((m) => ({
    value: m.model_id,
    textValue: m.effective.display_name && m.effective.display_name !== m.model_id ? `${m.effective.display_name} ${m.model_id}` : m.model_id,
    label:
      m.effective.display_name && m.effective.display_name !== m.model_id ? (
        <span className="flex min-w-0 items-baseline gap-8">
          {m.effective.display_name}
          <Text mono variant="small" tone="secondary">
            {m.model_id}
          </Text>
        </span>
      ) : (
        m.model_id
      ),
  }));
  const choice = (id: ChoiceFilterId): FilterChipChoice[] =>
    id === "key"
      ? keyOptions
      : id === "member"
        ? memberOptions
        : id === "model"
          ? modelOptions
          : id === "provider"
            ? PROVIDER_IDS.map((p) => ({ value: p, label: providerLabel(p) }))
            : id === "status"
              ? STATUSES.map((s) => ({ value: String(s), label: String(s) }))
              : id === "error"
                ? ERROR_KIND_IDS.map((k) => ({ value: k, label: errorKindLabel(k) }))
                : id === "stream"
                  ? [
                      { value: "true", label: t("logs.filter.streamed", "Streamed") },
                      { value: "false", label: t("logs.filter.notStreamed", "Not streamed") },
                    ]
                  : [
                      { value: "api", label: "API" },
                      { value: "playground", label: t("logs.source.playground", "Playground") },
                    ];
  const current = (id: ChoiceFilterId): string | undefined =>
    id === "status"
      ? filters.status
        ? String(filters.status)
        : undefined
      : id === "stream"
        ? filters.stream === undefined
          ? undefined
          : String(filters.stream)
        : filters[id];
  const set = (id: ChoiceFilterId, v: string | undefined) =>
    setSearch({ [id]: id === "status" ? (v ? Number(v) : undefined) : id === "stream" ? (v === undefined ? undefined : v === "true") : v, log: undefined });
  const active = FILTER_IDS.filter((id) => (id === "failed" ? !!filters.failed : current(id) !== undefined || adding === id));

  const canExport = rights.has("logs.export");
  const nothingToExport = logs.isSuccess && rows.length === 0;
  const unavailable = logs.error && isApiError(logs.error) && logs.error.type === "unavailable";

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("logs.title", "Request logs")}
        path="logs"
        actions={
          <Button
            size="sm"
            leadingIcon={icons.download}
            disabled={!canExport || nothingToExport}
            disabledReason={!canExport ? needs("logs.export") : nothingToExport ? t("logs.noMatch", "No requests match these filters") : undefined}
            onClick={() => showExportProgress() || setExporting(true)}
          >
            {common.export()}
          </Button>
        }
      />
      {canExport && (
        <ExportDialog
          org={org}
          project={project}
          filters={filters}
          names={{
            key: filters.key && keyName(filters.key),
            member: memberOptions.find((m) => m.value === filters.member)?.label,
            model: catalogue.data?.find((m) => m.model_id === filters.model)?.effective.display_name ?? undefined,
          }}
          canContent={rights.has("logs.content.view")}
          open={exporting}
          onOpenChange={setExporting}
        />
      )}
      <Toolbar aria-label={t("logs.filtersLabel", "Filter requests")}>
        <div className="shrink-0">
          <Select
            aria-label={t("logs.timeRange", "Time range")}
            size="sm"
            icon={icons.calendar}
            value={filters.from ? "custom" : (filters.range ?? "24h")}
            onValueChange={(v) =>
              v === "custom"
                ? setSearch({ from: utcDay(new Date()), to: utcDay(new Date()), range: undefined })
                : setSearch({ range: v === "24h" ? undefined : v, from: undefined, to: undefined })
            }
            options={[...ranges(), { value: "custom", label: t("logs.range.custom", "Custom range") }]}
          />
        </div>
        {filters.from && filters.to && (
          <div className="w-240 shrink-0">
            <DateRangePicker
              aria-label={t("logs.days", "Days")}
              size="sm"
              value={{ from: pickerDate(filters.from), to: pickerDate(filters.to) }}
              onValueChange={(r) => r?.from && setSearch({ from: pickedDay(r.from), to: pickedDay(r.to ?? r.from) })}
            />
          </div>
        )}
        {active.map((id) => {
          if (id === "failed")
            return (
              <FilterChip
                key={id}
                property={filterLabel(id)}
                operator={t("logs.filter.is", "is")}
                value={t("logs.filter.yes", "yes")}
                onRemove={() => setSearch({ failed: undefined })}
              />
            );
          const value = current(id);
          return (
            <FilterChip
              key={id}
              property={filterLabel(id)}
              operator={t("logs.filter.is", "is")}
              value={choice(id).find((c) => c.value === value)?.label ?? value ?? t("logs.filter.any", "any")}
              values={choice(id)}
              selected={value ? [value] : []}
              defaultValuesOpen={adding === id}
              onSelectedChange={(v) => {
                setAdding(null);
                set(id, v[0]);
              }}
              onRemove={() => {
                setAdding(null);
                set(id, undefined);
              }}
            />
          );
        })}
        <Menu>
          <Menu.Trigger>
            <Button variant="ghost" size="sm" leadingIcon={icons.add}>
              {t("logs.addFilter", "Add filter")}
            </Button>
          </Menu.Trigger>
          <Menu.Content>
            {FILTER_IDS.filter((id) => !active.includes(id)).map((id) => (
              <Menu.Item key={id} onClick={() => (id === "failed" ? setSearch({ failed: true }) : setAdding(id))}>
                {filterLabel(id)}
              </Menu.Item>
            ))}
          </Menu.Content>
        </Menu>
        <Toolbar.Spacer />
        <RequestIdSearch onOpen={(id) => setSearch({ log: id })} />
        <Button
          size="sm"
          variant={live ? "primary" : "secondary"}
          leadingIcon={icons.refresh}
          aria-pressed={!!live}
          onClick={() => setSearch({ live: live ? undefined : true })}
        >
          {t("logs.live", "Live")}
        </Button>
        <DisplayOptions
          rows={[
            {
              label: t("logs.density", "Density"),
              icon: icons.displayOptions,
              value: density,
              onChange: (v) => setDensity(v as "default" | "compact"),
              options: [
                { value: "default", label: t("logs.density.comfortable", "Comfortable") },
                { value: "compact", label: t("logs.density.compact", "Compact") },
              ],
            },
          ]}
          properties={{
            value: columns,
            onChange: (v) => {
              const next = v as ColumnId[];
              setColumns(next);
              writeStored(COLUMNS_KEY, JSON.stringify(next));
            },
            options: COLUMNS.map((c) => ({ value: c, label: columnLabel(c) })),
          }}
        />
      </Toolbar>
      <AppFrame.Body padding="none">
        <div ref={scroller} className="flex min-h-0 flex-col">
          {pending.length > 0 && (
            <div className="my-8 flex justify-center">
              <Button size="sm" onClick={showPending}>
                {t("logs.showNew", "{count, plural, one {Show # new request} other {Show # new requests}}", { count: pending.length })}
              </Button>
            </div>
          )}
          <DataTable
            aria-label={common.requests()}
            data={rows}
            columns={shownColumns}
            getRowId={(r) => r.id}
            density={density}
            loading={logs.isPending}
            error={logs.isError && !unavailable && rows.length === 0 ? t("logs.loadFailed", "Requests couldn't load") : undefined}
            onRetry={() => void logs.refetch()}
            empty={
              <Empty
                title={
                  unavailable
                    ? t("logs.unavailable", "Request logs are unavailable for now")
                    : active.length
                      ? t("logs.noMatch", "No requests match these filters")
                      : t("logs.emptyRange", "No requests in this range")
                }
                description={
                  unavailable
                    ? t("logs.unavailableHint", "Usage and limits still work.")
                    : active.length
                      ? t("logs.noMatchHint", "Remove a filter or widen the time range.")
                      : t("logs.emptyRangeHint", "Requests appear here as apps call the gateway with this project's keys.")
                }
              />
            }
            hasMore={logs.hasNextPage}
            loadingMore={logs.isFetchingNextPage}
            onLoadMore={() => void logs.fetchNextPage()}
            onRowClick={(r) => setSearch({ log: r.id })}
            activeRowId={log}
            freshRowIds={fresh}
          />
        </div>
      </AppFrame.Body>
      <LogInspector org={org} project={project} logId={log} keyName={keyName} onClose={() => setSearch({ log: undefined })} />
    </>
  );
}

const errorTone = (row: LogView) => (row.error_kind === "client_disconnected" ? "neutral" : row.status >= 500 ? "danger" : "warning");

function RequestIdSearch({ onOpen }: { onOpen: (id: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <div className="hidden w-200 md:block">
      <SearchInput
        aria-label={t("logs.openById", "Open a request by id")}
        placeholder={t("logs.column.requestId", "Request id")}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onClear={() => setValue("")}
        onKeyDown={(e) => {
          const id = value.trim();
          if (e.key !== "Enter" || !id) return;
          e.preventDefault();
          onOpen(id);
        }}
      />
    </div>
  );
}

function ModelCell({ row }: { row: LogView }) {
  const aliased = row.requested_model !== row.model && !row.requested_model.endsWith(`/${row.model}`);
  return (
    <span className="flex min-w-0 items-center gap-8">
      <Text mono variant="small" truncate>
        {row.model}
      </Text>
      {aliased && (
        <Text mono variant="small" tone="secondary" truncate className="hidden sm:block">
          {t("logs.fromModel", "from {model}", { model: row.requested_model })}
        </Text>
      )}
      {row.source === "playground" && (
        <Pill size="sm" variant="outline" className="hidden shrink-0 sm:inline-flex">
          {t("logs.source.playground", "Playground")}
        </Pill>
      )}
      {row.error_kind && (
        <Badge tone={errorTone(row)} className="hidden shrink-0 sm:inline-flex">
          {errorKindLabel(row.error_kind)}
        </Badge>
      )}
      {row.error_message && (
        <Text variant="small" tone="secondary" truncate className="hidden min-w-0 flex-1 sm:block">
          {row.error_message}
        </Text>
      )}
    </span>
  );
}

function TokensCell({ row }: { row: LogView }) {
  const u = row.usage;
  const detail = [
    u.cache_read_tokens && t("logs.tokens.cacheRead", "{count} cache read", { count: formatNumber(u.cache_read_tokens) }),
    u.cache_write_tokens && t("logs.tokens.cacheWrite", "{count} cache write", { count: formatNumber(u.cache_write_tokens) }),
    u.reasoning_tokens && t("logs.tokens.reasoning", "{count} reasoning", { count: formatNumber(u.reasoning_tokens) }),
  ].filter(Boolean);
  if (u.estimated) detail.unshift(t("logs.tokens.estimated", "Estimated: the stream ended before the provider's final count"));
  const text = `${u.estimated ? "≈ " : ""}${formatNumber(u.input_tokens)} / ${formatNumber(u.output_tokens)}`;
  return detail.length ? (
    <Tooltip content={detail.join(" · ")}>
      <Text tabular tabIndex={0}>
        {text}
      </Text>
    </Tooltip>
  ) : (
    <Text tabular>{text}</Text>
  );
}
