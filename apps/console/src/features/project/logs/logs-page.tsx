import { useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, DataTable, DisplayOptions, Empty, type FilterChipChoice, useProviderGroups, Toolbar, icons } from "@logicplanes/kaeso-ui";
import { useModelGroups } from "@logicplanes/kaeso-ui/playground";
import { isApiError } from "~/api/errors";
import { ENDPOINTS, logErrorsQuery, logsInfiniteQuery, type LogFilters } from "~/api/queries/logs";
import type { ErrorGroupView } from "~/api/generated/types";
import { CommonErrors, filtersOf, messagePrefix } from "./common-errors";
import { CLIENT_IDS, clientLabel } from "~/format/clients";
import { guardrailActionLabel } from "~/format/guardrails";
import { aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { formatDuration, formatPrice } from "@logicplanes/kaeso-ui/format";
import { DIALECTS, ERROR_KIND_IDS, dialectLabel, errorKindLabel } from "~/format/labels";
import { catalogueModelOption } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviderFilterOptions, useProviders } from "~/kaeso/providers";
import { showExportProgress } from "~/shell/export-job";
import { ProjectHeader } from "~/shell/project-header";
import { usePhone } from "~/shell/use-phone";
import { usePartyName } from "../keys/party";
import { COLUMNS, columnLabel, logColumns, logsLayout, providerInModel, storeColumns, storedColumns, type ColumnId } from "./columns";
import { ExportDialog } from "./export-dialog";
import { cacheLabel, outcomeLabel } from "./export";
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
// Amounts a request may cost at least, in USD, from a cent upwards.
const MIN_COSTS = ["0.01", "0.1", "1", "10"];
type FlagFilterId = "fell_back";
type FilterId =
  | "key"
  | "member"
  | "model"
  | "requested_model"
  | "provider"
  | "alias"
  | "ingress"
  | "egress"
  | "endpoint"
  | "status"
  | "error"
  | "stream"
  | "source"
  | "min_cost"
  | "failed"
  | "response_cache"
  | "min_duration_ms"
  | "tag"
  | "error_message"
  | "client"
  | "guardrail_action"
  | FlagFilterId;
type ChoiceFilterId = Exclude<FilterId, FlagFilterId>;
const isChoice = (id: string): id is ChoiceFilterId => id !== "fell_back";
const FILTER_IDS: FilterId[] = [
  "key",
  "member",
  "model",
  "requested_model",
  "provider",
  "alias",
  "fell_back",
  "ingress",
  "egress",
  "endpoint",
  "status",
  "failed",
  "error",
  "error_message",
  "stream",
  "source",
  "client",
  "response_cache",
  "min_duration_ms",
  "min_cost",
  "tag",
  "guardrail_action",
];
const GUARDRAIL_ACTIONS = ["allow", "block", "replace"] as const;
// Durations a request may take at least, in milliseconds.
const SLOWER_THAN = [1_000, 5_000, 10_000, 30_000, 60_000];
const VERDICTS = ["hit", "miss", "bypass"] as const;
const CHOICE_PATCH: { [K in ChoiceFilterId]: (v: string | undefined) => SearchPatch<LogsSearch> } = {
  key: (key) => ({ key }),
  member: (member) => ({ member }),
  model: (model) => ({ model }),
  requested_model: (requested_model) => ({ requested_model }),
  tag: (tag) => ({ tag }),
  error_message: (error_message) => ({ error_message }),
  client: (client) => ({ client }),
  guardrail_action: (v) => ({ guardrail_action: GUARDRAIL_ACTIONS.find((a) => a === v) }),
  provider: (provider) => ({ provider }),
  ingress: (ingress) => ({ ingress }),
  egress: (egress) => ({ egress }),
  alias: (alias) => ({ alias }),
  endpoint: (endpoint) => ({ endpoint }),
  min_cost: (min_cost) => ({ min_cost }),
  failed: (v) => ({ failed: v === undefined ? undefined : v === "true" }),
  response_cache: (v) => ({ response_cache: VERDICTS.find((c) => c === v) }),
  min_duration_ms: (v) => ({ min_duration_ms: v ? Number(v) : undefined }),
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
    requested_model: t("logs.filter.requestedModel", "Requested model"),
    tag: t("logs.filter.tag", "Tag"),
    error_message: t("logs.filter.errorMessage", "Error message"),
    client: t("logs.filter.client", "Client"),
    guardrail_action: t("logs.filter.guardrail", "Guardrail"),
    provider: common.provider(),
    ingress: t("logs.filter.appFormat", "App format"),
    egress: t("logs.filter.providerFormat", "Provider format"),
    status: common.status(),
    failed: t("logs.filter.outcome", "Outcome"),
    response_cache: t("logs.filter.responseCache", "Response cache"),
    min_duration_ms: t("logs.filter.slowerThan", "Slower than"),
    error: t("logs.filter.error", "Error kind"),
    stream: t("logs.filter.streaming", "Streaming"),
    source: t("logs.filter.source", "Source"),
    alias: t("logs.filter.alias", "Alias"),
    fell_back: t("logs.filter.fellBack", "Fell back"),
    endpoint: t("logs.filter.endpoint", "Endpoint"),
    min_cost: t("logs.filter.minCost", "Cost at least"),
  })[id];

// The names an app may send: each alias, and each model's id with and without its provider. A
// name from the URL that none of them is, such as a deleted alias, stays as the chip's value.
// The log has no list of every tag, so the filter offers those of the requests loaded, and the
// one in the address.
// Messages can't be listed, so the filter offers the start of the most common errors' and takes
// any text typed.
function messageChoices(groups: readonly ErrorGroupView[], current: string | undefined): FilterChipChoice[] {
  const texts = new Set(groups.flatMap((g) => messagePrefix(g.example) ?? []));
  if (current) texts.add(current);
  return [...texts].map((text) => ({ value: text, label: text }));
}

function tagChoices(rows: readonly { tags: string[] }[], current: string | undefined): FilterChipChoice[] {
  const tags = new Set(rows.flatMap((r) => r.tags));
  if (current) tags.add(current);
  return [...tags].sort().map((tag) => ({ value: tag, label: tag }));
}

function requestedNames(models: { provider: string; model_id: string }[], aliases: { name: string }[]): FilterChipChoice[] {
  const names = [...new Set([...aliases.map((a) => a.name), ...models.flatMap((m) => [m.model_id, `${m.provider}/${m.model_id}`])])];
  return names.map((n) => ({ value: n, label: n }));
}

export function LogsPage({ org, project, search }: { org: string; project: string; search: LogsSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const { log, live, kind: _kind, call: _call, call_status: _status, call_tool: _tool, call_server: _server, call_error: _error, ...filters } = search;
  const logs = useInfiniteQuery(logsInfiniteQuery(org, project, filters));
  const { keyName, keyOptions, memberOptions } = usePartyFilters(org, project);
  const catalogue = useQuery(catalogueQuery(org, project));
  // Someone who reads the log may not see the aliases; the chip then offers none.
  const aliases = useQuery({ ...aliasesQuery(org, project), meta: { quiet: true } });
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
  // Shown above the failures, and the message filter's choices; the same query either way.
  const commonErrors = useQuery({ ...logErrorsQuery(org, project, filters), enabled: filters.failed === true });
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
              : id === "alias"
                ? (aliases.data ?? []).map((a) => ({ value: a.name, label: a.name }))
                : id === "requested_model"
                  ? requestedNames(catalogue.data ?? [], aliases.data ?? [])
                  : id === "tag"
                    ? tagChoices(rows, filters.tag)
                    : id === "error_message"
                      ? messageChoices(commonErrors.data ?? [], filters.error_message)
                      : id === "client"
                        ? CLIENT_IDS.map((c) => ({ value: c, label: clientLabel(c) }))
                        : id === "guardrail_action"
                          ? GUARDRAIL_ACTIONS.map((a) => ({ value: a, label: guardrailActionLabel(a) }))
                          : id === "endpoint"
                            ? ENDPOINTS.map((e) => ({ value: e, label: e }))
                            : id === "min_cost"
                              ? MIN_COSTS.map((c) => ({ value: c, label: formatPrice(c) }))
                              : id === "failed"
                                ? [
                                    { value: "true", label: outcomeLabel(true) },
                                    { value: "false", label: outcomeLabel(false) },
                                  ]
                                : id === "response_cache"
                                  ? VERDICTS.map((v) => ({ value: v, label: cacheLabel(v) }))
                                  : id === "min_duration_ms"
                                    ? SLOWER_THAN.map((ms) => ({ value: String(ms), label: formatDuration(ms) }))
                                    : id === "stream"
                                      ? [
                                          { value: "true", label: t("logs.filter.streamed", "Streamed") },
                                          { value: "false", label: t("logs.filter.notStreamed", "Not streamed") },
                                        ]
                                      : [
                                          { value: "api", label: "API" },
                                          { value: "playground", label: t("logs.source.playground", "Playground") },
                                        ];
  const current = (id: ChoiceFilterId): string | undefined => {
    const v = filters[id];
    return v === undefined ? undefined : String(v);
  };
  // A duration from the URL that isn't one of the presets, such as an hour's 95th percentile.
  const valueLabel = (id: ChoiceFilterId, value: string): ReactNode =>
    id === "model"
      ? modelLabel(value)
      : id === "provider"
        ? catalog.name(value)
        : id === "min_duration_ms"
          ? formatDuration(Number(value))
          : (choice(id).find((c) => c.value === value)?.label ?? value);
  const filterDefs: FilterDef[] = FILTER_IDS.map((id) =>
    !isChoice(id)
      ? { id, label: filterLabel(id), flag: true, value: filters[id] ? "true" : undefined }
      : {
          id,
          label: filterLabel(id),
          value: current(id),
          valueLabel: (v) => valueLabel(id, v),
          ...(id === "model"
            ? { groups: modelGroups, width: "lg" as const }
            : id === "provider"
              ? { groups: providerGroups, width: "sm" as const }
              : {
                  values: choice(id),
                  searchable: id === "key" || id === "member" || id === "alias" || id === "endpoint" || id === "requested_model" || id === "tag",
                  ...(id === "error_message"
                    ? {
                        operator: t("logs.filter.contains", "contains"),
                        freeText: (text: string) => t("logs.filter.containsTyped", "Contains “{text}”", { text }),
                      }
                    : {}),
                }),
        },
  );
  const setFilter = (id: string, v: string | undefined) =>
    isChoice(id) ? setSearch({ ...CHOICE_PATCH[id](v), log: undefined }) : setSearch({ [id]: v ? true : undefined, log: undefined });
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
          {filters.failed === true && (
            <div className="mx-16 my-12">
              <CommonErrors
                org={org}
                project={project}
                filters={filters}
                providerName={catalog.name}
                onPick={(g) => setSearch({ ...filtersOf(g), log: undefined })}
              />
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
      <LogInspector
        org={org}
        project={project}
        logId={log}
        keyName={keyName}
        onClose={() => setSearch({ log: undefined })}
        onFilter={(filter) => setSearch(filter)}
      />
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
