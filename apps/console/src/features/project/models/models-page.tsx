import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { LoadFailed } from "~/features/org/load-failed";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Badge,
  Button,
  Empty,
  List,
  ModelLabel,
  Tag,
  Tabs,
  Popover,
  ProviderBadge,
  ProviderPicker,
  Select,
  Text,
  Toolbar,
  Tooltip,
  icons,
  type Capability,
  type ListGroup,
} from "@logicplanes/kaeso-ui";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import type { CatalogueModelView, RetiringModelView } from "~/api/generated/types";
import { api, data } from "~/api/client";
import { aliasesQuery, catalogueQuery, retiringQuery } from "~/api/queries/models";
import { projectCatalogue } from "~/api/queries/invalidation";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { useProviderOptions, useProviders } from "~/kaeso/providers";
import { useOrgPrices } from "~/kaeso/org-prices";
import type { Prices } from "~/format/prices";
import { usePhone } from "~/shell/use-phone";
import { monthLabel } from "~/format/period";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { AddModelsDialog } from "./add-models-dialog";
import { AliasesTab } from "./aliases-tab";
import { capabilities, capabilityOptions, contextLabel, dialectPill, overriddenFields, priceLabel } from "./model-fields";
import { ModelSheet } from "./model-sheet";
import { RetiringBanner, RetiringTab, shutdownLabel } from "./retiring";
import { checkFailureLabel } from "./availability";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";

export type ModelsSort = "model" | "price" | "release";
/** What a catalogue model's badges say: the provider doesn't serve it, KPMR deprecated it, or
 * KPMR no longer lists it (never a custom model). */
export type ModelStatus = "unavailable" | "deprecated" | "delisted";
const MODEL_STATUSES: readonly ModelStatus[] = ["unavailable", "deprecated", "delisted"];

const hasStatus = (m: CatalogueModelView, status: ModelStatus): boolean =>
  status === "unavailable" ? m.availability === "unavailable" : status === "deprecated" ? m.deprecated : !m.listed && !m.custom;

export type ModelsSearch = {
  tab?: "aliases" | "retiring";
  model?: string;
  provider?: string;
  cap?: Capability;
  status?: ModelStatus;
  overridden?: boolean;
  sort?: ModelsSort;
  add?: boolean;
};

// The same words as the badges.
const statusOptions = () => [
  { value: "unavailable", label: t("models.unavailable", "Unavailable") },
  { value: "deprecated", label: t("models.deprecated", "Deprecated") },
  { value: "delisted", label: t("models.delisted", "No longer listed") },
];

function sorted(list: CatalogueModelView[], sort: ModelsSort): CatalogueModelView[] {
  const by = {
    model: (a: CatalogueModelView, b: CatalogueModelView) => a.model_id.localeCompare(b.model_id),
    price: (a: CatalogueModelView, b: CatalogueModelView) =>
      Number(a.effective.input_price ?? Infinity) - Number(b.effective.input_price ?? Infinity) || a.model_id.localeCompare(b.model_id),
    release: (a: CatalogueModelView, b: CatalogueModelView) =>
      (b.effective.release_date ?? "").localeCompare(a.effective.release_date ?? "") || a.model_id.localeCompare(b.model_id),
  }[sort];
  return [...list].sort(by);
}

export function ModelsPage({ org, project, search }: { org: string; project: string; search: ModelsSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const canManage = rights.has("models.manage");
  const tab = search.tab ?? "catalogue";
  const [adding, setAdding] = useState(!!search.add && tab === "catalogue");
  const catalogue = useQuery(catalogueQuery(org, project));
  const aliases = useQuery(aliasesQuery(org, project));
  const retiring = useQuery(retiringQuery(org, project));
  const retiringById = useMemo(() => new Map((retiring.data?.data ?? []).map((m) => [m.id, m])), [retiring.data]);
  const prices = useOrgPrices(org);
  const sort = search.sort ?? "model";
  const reason = canManage ? undefined : needs("models.manage");

  const create =
    tab === "aliases"
      ? {
          label: t("models.createAlias", "Create alias"),
          run: () => void navigate({ to: "/orgs/$org/projects/$project/models/aliases/$alias", params: { org, project, alias: "new" } }),
        }
      : { label: t("models.addModels", "Add models"), run: () => setAdding(true) };
  usePageCreateAction({ ...create, disabledReason: reason });

  const setSearch = (patch: SearchPatch<ModelsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/models", params: { org, project }, search: patchSearch(search, patch, ["add"]), replace: true });
  };

  const all = useMemo(() => catalogue.data ?? [], [catalogue.data]);
  const providerIds = useMemo(() => [...new Set(all.map((m) => m.provider))], [all]);
  const providers = useProviderOptions(org, providerIds);
  const catalog = useProviders(org);
  const phone = usePhone();
  const filtered = useMemo(
    () =>
      all.filter(
        (m) =>
          (!search.provider || m.provider === search.provider) &&
          (!search.cap || capabilities(m.effective).includes(search.cap)) &&
          (!search.status || hasStatus(m, search.status)) &&
          (!search.overridden || overriddenFields(m).length > 0),
      ),
    [all, search.provider, search.cap, search.status, search.overridden],
  );
  const groups: ListGroup[] = useMemo(() => {
    const providers = [...new Set(filtered.map((m) => m.provider))].sort();
    return providers.map((p) => {
      const items = sorted(
        filtered.filter((m) => m.provider === p),
        sort,
      );
      return {
        id: p,
        title: catalog.name(p),
        glyph: <ProviderBadge provider={p} name={catalog.name(p)} size="sm" />,
        count: items.length,
        items: items.map((m) => ({
          id: m.id,
          textValue: m.model_id,
          title: (
            <ModelLabel
              provider={m.provider}
              providerName={catalog.name(m.provider)}
              model={m.model_id}
              mark={false}
              capabilities={capabilities(m.effective)}
              {...(m.effective.display_name && m.effective.display_name !== m.model_id ? { name: m.effective.display_name } : {})}
            />
          ),
          ...(phone && !MODEL_STATUSES.some((status) => hasStatus(m, status)) ? {} : { meta: <ModelMeta m={m} prices={prices} shutdown={retiringById.get(m.id)} /> }),
          date: m.effective.release_date ? monthLabel(m.effective.release_date) : undefined,
        })),
      };
    });
  }, [filtered, sort, catalog, retiringById, phone, prices]);

  const check = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/models/check", { params: { path: { org, project } } })),
    invalidates: projectCatalogue(org, project),
    onSuccess: (result) => {
      const first = result.failed[0];
      if (first) {
        toast.warning(checkFailureLabel(first, catalog.name(first.provider)), {
          description: t("models.checkFailedCount", "{count, plural, one {# model could not be checked.} other {# models could not be checked.}}", {
            count: result.failed.length,
          }),
        });
        return;
      }
      const unavailable = result.models.filter((m) => m.availability === "unavailable").length;
      if (!unavailable) {
        toast.success(t("models.checkedAll", "Checked {count, plural, one {# model} other {# models}}; all available", { count: result.models.length }));
        return;
      }
      toast.success(
        t("models.checkedSome", "Checked {count, plural, one {# model} other {# models}}; {unavailable} unavailable", {
          count: result.models.length,
          unavailable,
        }),
        { action: { label: t("models.showThem", "Show them"), onClick: () => setSearch({ status: "unavailable", model: undefined }) } },
      );
    },
  });

  const filtering = !!(search.provider || search.cap || search.status || search.overridden);
  const addButton = (
    <Button size="sm" variant="primary" leadingIcon={icons.add} disabled={!canManage} {...(reason ? { disabledReason: reason } : {})} onClick={create.run}>
      {create.label}
    </Button>
  );

  const filters = (stacked: boolean) => (
    <>
      <ProviderPicker
        appearance="select"
        size="sm"
        {...(stacked ? { className: "w-full" } : {})}
        aria-label={common.provider()}
        allLabel={t("models.allProviders", "All providers")}
        providers={providers}
        value={search.provider ?? null}
        onValueChange={(v) => setSearch({ provider: v ?? undefined })}
      />
      <Select
        aria-label={t("models.capability", "Capability")}
        size="sm"
        {...(stacked ? { fullWidth: true } : {})}
        value={search.cap ?? "any"}
        onValueChange={(v) => setSearch({ cap: capabilityOptions().find((c) => c.value === v)?.value })}
        options={[{ value: "any", label: t("models.anyCapability", "Any capability") }, ...capabilityOptions()]}
      />
      <Select
        aria-label={t("models.status", "Status")}
        size="sm"
        {...(stacked ? { fullWidth: true } : {})}
        value={search.status ?? "any"}
        onValueChange={(v) => setSearch({ status: MODEL_STATUSES.find((s) => s === v) })}
        options={[{ value: "any", label: t("models.anyStatus", "Any status") }, ...statusOptions()]}
      />
      <Button variant="selector" size="sm" pressed={!!search.overridden} onPressedChange={(v) => setSearch({ overridden: v || undefined })}>
        {t("models.onlyOverridden", "Only overridden")}
      </Button>
    </>
  );
  const activeFilters = [search.provider, search.cap, search.status, search.overridden].filter(Boolean).length;
  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={common.models()}
        path="models"
        tabs={
          <Tabs variant="pill" value={tab} onValueChange={(v) => setSearch({ tab: v === "aliases" || v === "retiring" ? v : undefined, model: undefined })}>
            <Tabs.List aria-label={common.models()}>
              <Tabs.Tab value="catalogue" {...(catalogue.data ? { count: catalogue.data.length } : {})}>
                {t("models.catalogue", "Catalogue")}
              </Tabs.Tab>
              <Tabs.Tab value="aliases" {...(aliases.data ? { count: aliases.data.length } : {})}>
                {t("models.aliases", "Aliases")}
              </Tabs.Tab>
              {(retiring.data?.data.length ?? 0) > 0 && (
                <Tabs.Tab value="retiring" count={retiring.data?.data.length}>
                  {t("models.retiring.title", "Retiring")}
                </Tabs.Tab>
              )}
            </Tabs.List>
          </Tabs>
        }
        actions={addButton}
      />
      {tab === "catalogue" ? (
        <>
          <Toolbar aria-label={t("models.filter", "Filter models")}>
            {phone ? (
              <Popover>
                <Popover.Trigger>
                  <Button size="sm" leadingIcon={icons.filter}>
                    {activeFilters ? t("models.filtersCount", "Filters · {count}", { count: activeFilters }) : t("models.filters", "Filters")}
                  </Button>
                </Popover.Trigger>
                <Popover.Content aria-label={t("models.filter", "Filter models")}>
                  <div className="flex w-280 max-w-full flex-col gap-12">{filters(true)}</div>
                </Popover.Content>
              </Popover>
            ) : (
              filters(false)
            )}
            <Toolbar.Spacer />
            <Button
              size="sm"
              leadingIcon={icons.refresh}
              loading={check.isPending}
              disabled={!canManage || all.length === 0}
              {...(reason ? { disabledReason: reason } : {})}
              onClick={() => check.mutate()}
            >
              {t("models.checkAvailability", "Check availability")}
            </Button>
            <Select
              aria-label={t("models.sort", "Sort")}
              size="sm"
              icon={icons.sort}
              value={sort}
              onValueChange={(v) => setSearch({ sort: v === "price" || v === "release" ? v : undefined })}
              options={[
                { value: "model", label: t("models.modelId", "Model id") },
                { value: "price", label: t("models.field.inputPrice", "Input price") },
                { value: "release", label: t("models.newestRelease", "Newest release") },
              ]}
            />
          </Toolbar>
          <AppFrame.Body padding="list">
            <RetiringBanner count={retiring.data?.soon ?? 0} onShow={() => setSearch({ tab: "retiring", model: undefined })} />
            {catalogue.isPending ? (
              <List aria-label={common.models()} items={[]} loading />
            ) : catalogue.isError ? (
              <LoadFailed title={t("models.catalogueFailed", "The catalogue couldn't load")} />
            ) : filtered.length === 0 ? (
              <Empty
                size="lg"
                title={filtering ? t("models.noMatches", "No models match these filters") : t("models.empty", "No models yet")}
                description={
                  filtering
                    ? t("models.noMatchesHint", "Clear the filters to see the whole catalogue.")
                    : t("models.emptyHint", "Requests may only name catalogue models; add them from KPMR with their prices.")
                }
                actions={
                  filtering ? (
                    <Button onClick={() => setSearch({ provider: undefined, cap: undefined, status: undefined, overridden: undefined })}>
                      {t("models.clearFilters", "Clear filters")}
                    </Button>
                  ) : canManage ? (
                    <Button variant="primary" onClick={() => setAdding(true)}>
                      {t("models.addModels", "Add models")}
                    </Button>
                  ) : undefined
                }
              />
            ) : (
              <List aria-label={common.models()} groups={groups} onOpen={(item) => setSearch({ model: item.id })} />
            )}
          </AppFrame.Body>
        </>
      ) : tab === "aliases" ? (
        <AppFrame.Body padding="list">
          <AliasesTab org={org} project={project} canManage={canManage} />
        </AppFrame.Body>
      ) : (
        <AppFrame.Body padding="list">
          <RetiringTab org={org} project={project} canManage={canManage} />
        </AppFrame.Body>
      )}
      <ModelSheet org={org} project={project} modelId={search.model} onClose={() => setSearch({ model: undefined })} />
      <AddModelsDialog org={org} project={project} open={adding} onClose={() => setAdding(false)} />
    </>
  );
}

function ModelMeta({ m, prices, shutdown }: { m: CatalogueModelView; prices: Prices; shutdown?: RetiringModelView | undefined }) {
  const f = m.effective;
  const overridden = overriddenFields(m).length;
  const unit =
    f.output_price && Number(f.output_price) === 0
      ? t("models.perMillionInput", "Per million input tokens")
      : t("models.perMillionInputOutput", "Per million input / output tokens");
  const read = f.cache_read_price ? prices.unit(f.cache_read_price) : null;
  const write = f.cache_write_price ? prices.unit(f.cache_write_price) : null;
  const cache =
    read && write
      ? t("models.cacheReadWrite", "cache read {read}, cache write {write}", { read, write })
      : read
        ? t("models.cacheRead", "cache read {read}", { read })
        : write
          ? t("models.cacheWrite", "cache write {write}", { write })
          : null;
  const unavailable = m.availability === "unavailable" && <Badge tone="danger">{t("models.unavailable", "Unavailable")}</Badge>;
  return (
    <span className="flex items-center gap-8">
      {unavailable}
      {m.line_target && (
        <span className="hidden md:contents">
          <Tooltip content={t("models.lines.badgeHint", "A model line; requests go to {model}", { model: m.line_target })}>
            <Badge tabIndex={0}>{t("models.lines.badge", "Line → {model}", { model: m.line_target })}</Badge>
          </Tooltip>
        </span>
      )}
      {shutdown && shutdownLabel(shutdown) ? (
        <Tooltip
          content={
            shutdown.lifecycle.replacement
              ? t("models.retiring.replaceWith", "Replace with {model}", { model: shutdown.lifecycle.replacement })
              : t("models.retiring.noReplacement", "No replacement named")
          }
        >
          <Badge tone={shutdown.state === "retired" ? "danger" : "warning"} tabIndex={0}>
            {shutdownLabel(shutdown)}
          </Badge>
        </Tooltip>
      ) : (
        m.deprecated && <Badge tone="warning">{t("models.deprecated", "Deprecated")}</Badge>
      )}
      {!m.listed && !m.custom && <Badge tone="warning">{t("models.delisted", "No longer listed")}</Badge>}
      <span className="hidden md:contents">
        {m.custom ? (
          <Badge>{t("models.custom", "Custom")}</Badge>
        ) : (
          overridden > 0 && <Badge>{t("models.overriddenCount", "Overridden {count}", { count: overridden })}</Badge>
        )}
        {m.aliases.slice(0, 2).map((a) => (
          <Tag key={a} size="sm">
            {a}
          </Tag>
        ))}
        {m.aliases.length > 2 && (
          <Text variant="small" tone="secondary">
            {t("models.moreAliases", "+{count}", { count: m.aliases.length - 2 })}
          </Text>
        )}
        <Tag size="sm" variant="outline">
          {dialectPill(m.egress_dialect, !!f.decisions?.only)}
        </Tag>
        {f.context_limit ? (
          <Tooltip content={t("models.contextHint", "Context window: {tokens} tokens", { tokens: formatNumber(f.context_limit) })}>
            <Text variant="small" tone="secondary" tabular tabIndex={0} className="min-w-48 text-right">
              {contextLabel(f.context_limit)}
            </Text>
          </Tooltip>
        ) : (
          <Text variant="small" tone="secondary" tabular className="min-w-48 text-right">
            {contextLabel(f.context_limit)}
          </Text>
        )}
        <Tooltip content={cache ? t("models.priceWithCache", "{unit}; {cache}", { unit, cache }) : unit}>
          <Text variant="small" tone="secondary" tabular tabIndex={0} className="min-w-96 text-right">
            {priceLabel(f, prices)}
          </Text>
        </Tooltip>
      </span>
    </span>
  );
}
