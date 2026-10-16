import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Button,
  DataTable,
  DateRangePicker,
  DisplayOptions,
  Empty,
  FilterChip,
  type FilterChipChoice,
  Menu,
  SearchInput,
  Select,
  Text,
  Toolbar,
  icons,
} from "@logicplanes/kaeso-ui";
import { isApiError } from "~/api/errors";
import { allKeysQuery } from "~/api/queries/keys";
import { logsInfiniteQuery, type LogFilters, type LogRange } from "~/api/queries/logs";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { ERROR_KIND_IDS, PROVIDER_IDS, errorKindLabel, providerLabel } from "~/format/labels";
import { utcDay } from "~/format/period";
import { pickedDay, pickerDate } from "~/format/picked-day";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyName } from "../keys/party";
import { COLUMNS, columnLabel, logColumns, storeColumns, storedColumns, type ColumnId } from "./columns";
import { ExportDialog } from "./export-dialog";
import { scrollParentOf, useLiveLogs } from "./live";
import { LogInspector } from "./log-inspector";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";

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
const CHOICE_PATCH: { [K in ChoiceFilterId]: (v: string | undefined) => SearchPatch<LogsSearch> } = {
  key: (key) => ({ key }),
  member: (member) => ({ member }),
  model: (model) => ({ model }),
  provider: (provider) => ({ provider }),
  error: (error) => ({ error }),
  status: (v) => ({ status: v ? Number(v) : undefined }),
  stream: (v) => ({ stream: v === undefined ? undefined : v === "true" }),
  source: (v) => ({ source: v === "api" || v === "playground" ? v : undefined }),
};
const filterLabel = (id: FilterId): string =>
  ({
    key: t("logs.filter.key", "Key"),
    member: t("logs.filter.member", "Member"),
    model: common.model(),
    provider: common.provider(),
    status: common.status(),
    failed: t("logs.filter.failed", "Failed only"),
    error: t("logs.filter.error", "Error kind"),
    stream: t("logs.filter.streaming", "Streaming"),
    source: t("logs.filter.source", "Source"),
  })[id];

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

  const setSearch = (patch: SearchPatch<LogsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: patchSearch(search, patch), replace: true });
  };

  const base = useMemo(() => logs.data?.pages.flatMap((p) => p.data) ?? [], [logs.data]);
  const { rows, pending, fresh, showPending } = useLiveLogs({ org, project, filters, live: !!live, base, scroller });
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = logs;
  const loadMore = useEffectEvent(() => void fetchNextPage());
  useEffect(() => {
    const more = scroller.current?.querySelector("nav");
    if (!more || !hasNextPage || isFetchingNextPage) return;
    const seen = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && loadMore(), {
      root: scrollParentOf(scroller.current),
      rootMargin: "800px 0px",
    });
    seen.observe(more);
    return () => seen.disconnect();
  }, [hasNextPage, isFetchingNextPage]);

  const step = useEffectEvent((e: KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey || (e.target as HTMLElement).closest("input, textarea, [contenteditable], [role=combobox]")) return;
    if (e.key !== "j" && e.key !== "k") return;
    const i = rows.findIndex((r) => r.id === log);
    const next = rows[e.key === "j" ? i + 1 : i - 1];
    if (next) {
      e.preventDefault();
      setSearch({ log: next.id });
    }
  });
  useEffect(() => {
    if (!log) return;
    window.addEventListener("keydown", step);
    return () => window.removeEventListener("keydown", step);
  }, [log]);

  const keyName = (id: string) => keys.data?.find((k) => k.id === id)?.name ?? id;
  const spansDays = filters.from ? filters.from !== filters.to : filters.range !== "1h";
  const all = logColumns({ spansDays, keyName, partyName });
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
  const set = (id: ChoiceFilterId, v: string | undefined) => setSearch({ ...CHOICE_PATCH[id](v), log: undefined });
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
                : setSearch({ range: ranges().find((r) => r.value === v && v !== "24h")?.value, from: undefined, to: undefined })
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
              storeColumns(next);
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
