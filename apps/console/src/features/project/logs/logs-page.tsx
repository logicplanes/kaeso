import { useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, DataTable, DisplayOptions, Empty, type FilterChipChoice, useProviderGroups, Toolbar, icons } from "@logicplanes/kaeso-ui";
import { useModelGroups } from "@logicplanes/kaeso-ui/playground";
import { isApiError } from "~/api/errors";
import { logsInfiniteQuery, type LogFilters } from "~/api/queries/logs";
import { catalogueQuery } from "~/api/queries/models";
import { DIALECTS, ERROR_KIND_IDS, dialectLabel, errorKindLabel } from "~/format/labels";
import { catalogueModelOption } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviderFilterOptions, useProviders } from "~/kaeso/providers";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePhone } from "~/shell/use-phone";
import { usePartyName } from "../keys/party";
import { COLUMNS, columnLabel, logColumns, logsLayout, providerInModel, storeColumns, storedColumns, type ColumnId } from "./columns";
import { ExportDialog } from "./export-dialog";
import { useLiveLogs } from "./live";
import { LogInspector } from "./log-inspector";
import { KindSwitch } from "./tool-calls";
import {
  FilterBar,
  IdSearch,
  LiveButton,
  RangeControls,
  useLoadMoreNearEnd,
  usePartyFilters,
  useRowStepping,
  densityRow,
  type Density,
  type FilterDef,
} from "./toolbar";
import { useGatewayErrors } from "./messages";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";

export type LogsSearch = LogFilters & {
  log?: string;
  live?: boolean;
  kind?: "tools";
  call?: string;
  call_status?: "ok" | "error" | "refused";
  call_tool?: string;
  call_server?: string;
  call_error?: string;
};

const STATUSES = [200, 400, 401, 403, 404, 408, 413, 422, 429, 499, 500, 502, 503, 504, 529];
type FilterId = "key" | "member" | "model" | "provider" | "ingress" | "egress" | "status" | "error" | "stream" | "source" | "failed";
type ChoiceFilterId = Exclude<FilterId, "failed">;
const isChoice = (id: string): id is ChoiceFilterId => id !== "failed";
const FILTER_IDS: FilterId[] = ["key", "member", "model", "provider", "ingress", "egress", "status", "failed", "error", "stream", "source"];
const CHOICE_PATCH: { [K in ChoiceFilterId]: (v: string | undefined) => SearchPatch<LogsSearch> } = {
  key: (key) => ({ key }),
  member: (member) => ({ member }),
  model: (model) => ({ model }),
  provider: (provider) => ({ provider }),
  ingress: (ingress) => ({ ingress }),
  egress: (egress) => ({ egress }),
  error: (error) => ({ error }),
  status: (v) => ({ status: v ? Number(v) : undefined }),
  stream: (v) => ({ stream: v === undefined ? undefined : v === "true" }),
  source: (v) => ({ source: v as LogFilters["source"] }),
};
const filterLabel = (id: FilterId): string =>
  ({
    key: t("logs.filter.key", "Key"),
    member: t("logs.filter.member", "Member"),
    model: common.model(),
    provider: common.provider(),
    ingress: t("logs.filter.appFormat", "App format"),
    egress: t("logs.filter.providerFormat", "Provider format"),
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
  const { log, live, kind: _kind, call: _call, call_status: _status, call_tool: _tool, call_server: _server, call_error: _error, ...filters } = search;
  const logs = useInfiniteQuery(logsInfiniteQuery(org, project, filters));
  const { keyName, keyOptions, memberOptions } = usePartyFilters(org, project);
  const catalogue = useQuery(catalogueQuery(org, project));
  const providers = useProviderFilterOptions(org);
  const catalog = useProviders(org);
  const partyName = usePartyName(org, project);
  const [columns, setColumns] = useState<ColumnId[]>(storedColumns);
  const [density, setDensity] = useState<Density>("default");
  const [squeezed, setSqueezed] = useState<string[]>([]);
  const [exporting, setExporting] = useState(false);
  const scroller = useRef<HTMLDivElement | null>(null);
  const width = useWidth(scroller);

  const setSearch = (patch: SearchPatch<LogsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: patchSearch(search, patch), replace: true });
  };

  const base = useMemo(() => logs.data?.pages.flatMap((p) => p.data) ?? [], [logs.data]);
  const { rows, pending, fresh, cut, showPending } = useLiveLogs({ org, project, filters, live: !!live, base, scroller });
  const hasNextPage = logs.hasNextPage && !cut;
  useLoadMoreNearEnd(scroller, { ...logs, hasNextPage });
  useRowStepping(rows, log, (id) => setSearch({ log: id }));

  const spansDays = filters.from ? filters.from !== filters.to : filters.range !== "1h";
  const phone = usePhone();
  const errors = useGatewayErrors();
  const layout = logsLayout(width, phone, spansDays, columns);
  const withProvider = providerInModel(layout, columns);
  const all = logColumns({ spansDays, layout, withProvider, keyName, partyName, providerName: catalog.name, errors });
  const shownColumns = COLUMNS.filter((c) => columns.includes(c) && !(withProvider && c === "provider")).map((c) => all[c]);

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
            : id === "ingress" || id === "egress"
              ? Object.keys(DIALECTS).map((d) => ({ value: d, label: dialectLabel(d) }))
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
  const valueLabel = (id: ChoiceFilterId, value: string): ReactNode =>
    id === "model" ? modelLabel(value) : id === "provider" ? catalog.name(value) : (choice(id).find((c) => c.value === value)?.label ?? value);
  const filterDefs: FilterDef[] = FILTER_IDS.map((id) =>
    !isChoice(id)
      ? { id, label: filterLabel(id), flag: true, value: filters.failed ? "true" : undefined }
      : {
          id,
          label: filterLabel(id),
          value: current(id),
          valueLabel: (v) => valueLabel(id, v),
          ...(id === "model"
            ? { groups: modelGroups, width: "lg" as const }
            : id === "provider"
              ? { groups: providerGroups, width: "sm" as const }
              : { values: choice(id), searchable: id === "key" || id === "member" }),
        },
  );
  const setFilter = (id: string, v: string | undefined) =>
    isChoice(id) ? setSearch({ ...CHOICE_PATCH[id](v), log: undefined }) : setSearch({ failed: v ? true : undefined });
  const filtered = filterDefs.some((f) => f.value !== undefined);

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
        <KindSwitch value="models" onChange={(kind) => setSearch({ kind, log: undefined })} />
        <RangeControls value={filters} onChange={setSearch} />
        <FilterBar filters={filterDefs} onChange={setFilter} compact={phone} />
        <Toolbar.Spacer />
        <IdSearch
          label={t("logs.openById", "Open a request by id")}
          placeholder={t("logs.column.requestId", "Request id")}
          onOpen={(id) => setSearch({ log: id })}
        />
        <LiveButton live={!!live} onChange={(on) => setSearch({ live: on || undefined })} />
        <DisplayOptions
          rows={[densityRow(density, setDensity)]}
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
                    : filtered
                      ? t("logs.noMatch", "No requests match these filters")
                      : t("logs.emptyRange", "No requests in this range")
                }
                description={
                  unavailable
                    ? t("logs.unavailableHint", "Usage and limits still work.")
                    : filtered
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
