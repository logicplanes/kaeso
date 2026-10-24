import { useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
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
  IconButton,
  Menu,
  SearchInput,
  useProviderGroups,
  Select,
  Toolbar,
  icons,
} from "@logicplanes/kaeso-ui";
import { useModelGroups } from "@logicplanes/kaeso-ui/playground";
import { isApiError } from "~/api/errors";
import { allKeysQuery } from "~/api/queries/keys";
import { logsInfiniteQuery, type LogFilters, type LogRange } from "~/api/queries/logs";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { ERROR_KIND_IDS, errorKindLabel } from "~/format/labels";
import { catalogueModelOption } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviderFilterOptions, useProviders } from "~/kaeso/providers";
import { utcDay } from "~/format/period";
import { pickedDay, pickerDate } from "~/format/picked-day";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePhone } from "~/shell/use-phone";
import { keyLabel, usePartyName } from "../keys/party";
import { COLUMNS, columnLabel, logColumns, logsLayout, providerInModel, storeColumns, storedColumns, type ColumnId } from "./columns";
import { ExportDialog } from "./export-dialog";
import { scrollParentOf, useLiveLogs } from "./live";
import { LogInspector } from "./log-inspector";
import { useGatewayErrors } from "./messages";
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
  const providers = useProviderFilterOptions(org);
  const catalog = useProviders(org);
  const partyName = usePartyName(org, project);
  const [columns, setColumns] = useState<ColumnId[]>(storedColumns);
  const [density, setDensity] = useState<"default" | "compact">("default");
  const [adding, setAdding] = useState<FilterId | null>(null);
  const [added, setAdded] = useState<FilterId[]>([]);
  const [squeezed, setSqueezed] = useState<string[]>([]);
  const [exporting, setExporting] = useState(false);
  const scroller = useRef<HTMLDivElement | null>(null);
  const width = useWidth(scroller);

  const setSearch = (patch: SearchPatch<LogsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: patchSearch(search, patch), replace: true });
  };

  const base = useMemo(() => logs.data?.pages.flatMap((p) => p.data) ?? [], [logs.data]);
  const { rows, pending, fresh, cut, showPending } = useLiveLogs({ org, project, filters, live: !!live, base, scroller });
  const { isFetchingNextPage, fetchNextPage } = logs;
  const hasNextPage = logs.hasNextPage && !cut;
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

  const keyName = (id: string) => {
    const key = keys.data?.find((k) => k.id === id);
    return key ? keyLabel(key) : id;
  };
  const spansDays = filters.from ? filters.from !== filters.to : filters.range !== "1h";
  const phone = usePhone();
  const errors = useGatewayErrors();
  const layout = logsLayout(width, phone, spansDays, columns);
  const withProvider = providerInModel(layout, columns);
  const all = logColumns({ spansDays, layout, withProvider, keyName, partyName, providerName: catalog.name, errors });
  const shownColumns = COLUMNS.filter((c) => columns.includes(c) && !(withProvider && c === "provider")).map((c) => all[c]);

  const keyOptions = (keys.data ?? []).map((k) => ({ value: k.id, label: keyLabel(k) }));
  const memberOptions = [
    ...(members.data ?? []).map((m) => ({ value: `user:${m.user.id}`, label: m.user.name })),
    ...(accounts.data ?? []).map((a) => ({ value: `sa:${a.id}`, label: t("logs.filter.serviceAccount", "{name} (service account)", { name: a.name }) })),
  ];
  const modelGroups = useModelGroups(
    useMemo(
      () =>
        (catalogue.data ?? [])
          .filter((m) => !filters.provider || m.provider === filters.provider)
          .map((m) => {
            const { deprecated: _, ...option } = catalogueModelOption(m);
            return { ...option, value: m.model_id };
          }),
      [catalogue.data, filters.provider],
    ),
    {
      providers: useModelPickerProviders(
        org,
        useMemo(() => [...new Set((catalogue.data ?? []).map((m) => m.provider))], [catalogue.data]),
      ),
    },
  );
  const modelLabel = (id: string) => {
    const m = catalogue.data?.find((c) => c.model_id === id);
    return m?.effective.display_name && m.effective.display_name !== id ? m.effective.display_name : id;
  };
  const providerGroups = useProviderGroups(useMemo(() => providers.map(({ hint: _, ...p }) => p), [providers]));
  const choice = (id: ChoiceFilterId): FilterChipChoice[] =>
    id === "key"
      ? keyOptions
      : id === "member"
        ? memberOptions
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
  const present = FILTER_IDS.filter((id) => (id === "failed" ? !!filters.failed : current(id) !== undefined || adding === id));
  const active = [...present.filter((id) => !added.includes(id)), ...added.filter((id) => present.includes(id))];
  const startAdding = (id: FilterId) => {
    setAdded((a) => [...a.filter((x) => x !== id), id]);
    if (id === "failed") setSearch({ failed: true });
    else setAdding(id);
  };
  const valueLabel = (id: ChoiceFilterId, value: string): ReactNode =>
    id === "model" ? modelLabel(value) : id === "provider" ? catalog.name(value) : (choice(id).find((c) => c.value === value)?.label ?? value);

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
            provider: filters.provider && catalog.name(filters.provider),
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
          const picked = { current: false };
          return (
            <FilterChip
              key={id}
              property={filterLabel(id)}
              operator={t("logs.filter.is", "is")}
              value={value ? valueLabel(id, value) : t("logs.filter.any", "any")}
              {...(id === "model"
                ? { groups: modelGroups, width: "lg" as const }
                : id === "provider"
                  ? { groups: providerGroups, width: "sm" as const }
                  : { values: choice(id), searchable: id === "key" || id === "member" })}
              selected={value ? [value] : []}
              defaultValuesOpen={adding === id}
              onValuesOpenChange={(open) => {
                if (!open && !picked.current && adding === id) setAdding(null);
              }}
              onSelectedChange={(v) => {
                picked.current = true;
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
            {phone ? (
              <IconButton icon={icons.add} size="sm" aria-label={t("logs.addFilter", "Add filter")} />
            ) : (
              <Button variant="ghost" size="sm" leadingIcon={icons.add}>
                {t("logs.addFilter", "Add filter")}
              </Button>
            )}
          </Menu.Trigger>
          <Menu.Content>
            {FILTER_IDS.filter((id) => !active.includes(id)).map((id) => (
              <Menu.Item key={id} onClick={() => startAdding(id)}>
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
            hidden: squeezed,
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
            hasMore={hasNextPage}
            loadingMore={logs.isFetchingNextPage}
            onLoadMore={() => void logs.fetchNextPage()}
            onRowClick={(r) => setSearch({ log: r.id })}
            activeRowId={log}
            freshRowIds={fresh}
            onHiddenColumnsChange={setSqueezed}
          />
        </div>
      </AppFrame.Body>
      <LogInspector org={org} project={project} logId={log} keyName={keyName} onClose={() => setSearch({ log: undefined })} />
    </>
  );
}

function useWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setWidth(Math.round(el.getBoundingClientRect().width));
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(el);
    return () => seen.disconnect();
  }, [ref]);
  return width;
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
