import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Badge,
  Button,
  Empty,
  List,
  ModelLabel,
  Pill,
  PillTabs,
  ProviderBadge,
  Select,
  Switch,
  Text,
  Toolbar,
  Tooltip,
  icons,
  type Capability,
  type ListGroup,
} from "@logicplanes/kaeso-ui";
import { formatUSD } from "@logicplanes/kaeso-ui/format";
import type { CatalogueModelView } from "~/api/generated/types";
import { api, data } from "~/api/client";
import { aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { providerLabel } from "~/format/labels";
import { monthLabel } from "~/format/period";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { AddModelsDialog } from "./add-models-dialog";
import { AliasesTab } from "./aliases-tab";
import { capabilities, capabilityOptions, contextLabel, dialectPill, overriddenFields, priceLabel } from "./model-fields";
import { ModelSheet } from "./model-sheet";
import { checkFailureLabel } from "./availability";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";

export type ModelsSort = "model" | "price" | "release";
export type ModelsSearch = {
  tab?: "aliases";
  model?: string;
  provider?: string;
  cap?: Capability;
  overridden?: boolean;
  sort?: ModelsSort;
  add?: boolean;
};

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
  const [creatingAlias, setCreatingAlias] = useState(!!search.add && tab === "aliases");
  const catalogue = useQuery(catalogueQuery(org, project));
  const aliases = useQuery(aliasesQuery(org, project));
  const sort = search.sort ?? "model";
  const reason = canManage ? undefined : needs("models.manage");

  const create =
    tab === "aliases"
      ? { label: t("models.createAlias", "Create alias"), run: () => setCreatingAlias(true) }
      : { label: t("models.addModels", "Add models"), run: () => setAdding(true) };
  usePageCreateAction({ ...create, disabledReason: reason });

  const setSearch = (patch: SearchPatch<ModelsSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/models", params: { org, project }, search: patchSearch(search, patch, ["add"]), replace: true });
  };

  const all = useMemo(() => catalogue.data ?? [], [catalogue.data]);
  const filtered = useMemo(
    () =>
      all.filter(
        (m) =>
          (!search.provider || m.provider === search.provider) &&
          (!search.cap || capabilities(m.effective).includes(search.cap)) &&
          (!search.overridden || overriddenFields(m).length > 0),
      ),
    [all, search.provider, search.cap, search.overridden],
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
        title: providerLabel(p),
        glyph: <ProviderBadge provider={p} size="sm" />,
        count: items.length,
        items: items.map((m) => ({
          id: m.id,
          textValue: m.model_id,
          title: (
            <ModelLabel
              provider={m.provider}
              model={m.model_id}
              capabilities={capabilities(m.effective)}
              {...(m.effective.display_name && m.effective.display_name !== m.model_id ? { name: m.effective.display_name } : {})}
            />
          ),
          meta: <ModelMeta m={m} />,
          date: m.effective.release_date ? monthLabel(m.effective.release_date) : undefined,
        })),
      };
    });
  }, [filtered, sort]);

  const check = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/models/check", { params: { path: { org, project } } })),
    invalidates: [["orgs", org, "projects", project, "models"]],
    onSuccess: (result) => {
      const first = result.failed[0];
      if (first) {
        toast.warning(checkFailureLabel(first), {
          description: t("models.checkFailedCount", "{count, plural, one {# model could not be checked.} other {# models could not be checked.}}", {
            count: result.failed.length,
          }),
        });
        return;
      }
      const unavailable = result.models.filter((m) => m.availability === "unavailable").length;
      toast.success(
        unavailable
          ? t("models.checkedSome", "Checked {count, plural, one {# model} other {# models}}; {unavailable} unavailable", {
              count: result.models.length,
              unavailable,
            })
          : t("models.checkedAll", "Checked {count, plural, one {# model} other {# models}}; all available", { count: result.models.length }),
      );
    },
  });

  const filtering = !!(search.provider || search.cap || search.overridden);
  const addButton = (
    <Button size="sm" variant="primary" leadingIcon={icons.add} disabled={!canManage} {...(reason ? { disabledReason: reason } : {})} onClick={create.run}>
      {create.label}
    </Button>
  );

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={common.models()}
        path="models"
        tabs={
          <PillTabs value={tab} onValueChange={(v) => setSearch({ tab: v === "aliases" ? "aliases" : undefined, model: undefined })}>
            <PillTabs.List aria-label={common.models()}>
              <PillTabs.Tab value="catalogue" {...(catalogue.data ? { count: catalogue.data.length } : {})}>
                {t("models.catalogue", "Catalogue")}
              </PillTabs.Tab>
              <PillTabs.Tab value="aliases" {...(aliases.data ? { count: aliases.data.length } : {})}>
                {t("models.aliases", "Aliases")}
              </PillTabs.Tab>
            </PillTabs.List>
          </PillTabs>
        }
        actions={addButton}
      />
      {tab === "catalogue" ? (
        <>
          <Toolbar aria-label={t("models.filter", "Filter models")}>
            <Select
              aria-label={common.provider()}
              size="sm"
              value={search.provider ?? "all"}
              onValueChange={(v) => setSearch({ provider: v === "all" ? undefined : v })}
              options={[
                { value: "all", label: t("models.allProviders", "All providers") },
                ...[...new Set(all.map((m) => m.provider))].sort().map((p) => ({ value: p, label: providerLabel(p) })),
              ]}
            />
            <Select
              aria-label={t("models.capability", "Capability")}
              size="sm"
              value={search.cap ?? "any"}
              onValueChange={(v) => setSearch({ cap: capabilityOptions().find((c) => c.value === v)?.value })}
              options={[{ value: "any", label: t("models.anyCapability", "Any capability") }, ...capabilityOptions()]}
            />
            <Switch
              size="sm"
              label={t("models.onlyOverridden", "Only overridden")}
              checked={!!search.overridden}
              onCheckedChange={(v) => setSearch({ overridden: v || undefined })}
            />
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
            {catalogue.isPending ? (
              <List aria-label={common.models()} items={[]} loading />
            ) : filtered.length === 0 ? (
              <Empty
                size="lg"
                title={filtering ? t("models.noMatches", "No models match these filters") : t("models.empty", "No models yet")}
                description={
                  filtering
                    ? t("models.noMatchesHint", "Clear the filters to see the whole catalogue.")
                    : t("models.emptyHint", "Requests may only name catalogue models; add them from models.dev with their prices.")
                }
                actions={
                  filtering ? (
                    <Button onClick={() => setSearch({ provider: undefined, cap: undefined, overridden: undefined })}>
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
      ) : (
        <AppFrame.Body padding="list">
          <AliasesTab org={org} project={project} canManage={canManage} creating={creatingAlias} onCreatingChange={setCreatingAlias} />
        </AppFrame.Body>
      )}
      <ModelSheet org={org} project={project} modelId={search.model} onClose={() => setSearch({ model: undefined })} />
      <AddModelsDialog org={org} project={project} open={adding} onClose={() => setAdding(false)} />
    </>
  );
}

function ModelMeta({ m }: { m: CatalogueModelView }) {
  const f = m.effective;
  const overridden = overriddenFields(m).length;
  const unit =
    f.output_price && Number(f.output_price) === 0
      ? t("models.perMillionInput", "Per million input tokens")
      : t("models.perMillionInputOutput", "Per million input / output tokens");
  const read = f.cache_read_price ? formatUSD(f.cache_read_price) : null;
  const write = f.cache_write_price ? formatUSD(f.cache_write_price) : null;
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
    <>
      {unavailable && <span className="md:hidden">{unavailable}</span>}
      <span className="hidden items-center gap-8 md:flex">
        {unavailable}
        {m.deprecated && <Badge tone="warning">{t("models.deprecated", "Deprecated")}</Badge>}
        {!m.listed && !m.custom && <Badge tone="warning">{t("models.delisted", "No longer listed")}</Badge>}
        {m.custom ? (
          <Badge>{t("models.custom", "Custom")}</Badge>
        ) : (
          overridden > 0 && <Badge>{t("models.overriddenCount", "Overridden {count}", { count: overridden })}</Badge>
        )}
        {m.aliases.slice(0, 2).map((a) => (
          <Pill key={a} size="sm">
            {a}
          </Pill>
        ))}
        {m.aliases.length > 2 && (
          <Text variant="small" tone="secondary">
            {t("models.moreAliases", "+{count}", { count: m.aliases.length - 2 })}
          </Text>
        )}
        <Pill size="sm" variant="outline">
          {dialectPill(m.egress_dialect)}
        </Pill>
        <Text variant="small" tone="secondary" tabular className="min-w-48 text-right">
          {contextLabel(f.context_limit)}
        </Text>
        <Tooltip content={cache ? t("models.priceWithCache", "{unit}; {cache}", { unit, cache }) : unit}>
          <Text variant="small" tone="secondary" tabular tabIndex={0} className="min-w-96 text-right">
            {priceLabel(f)}
          </Text>
        </Tooltip>
      </span>
    </>
  );
}
