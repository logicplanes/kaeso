import { Fragment, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Button, DialogScope, Empty, KeyValue, Link, Menu, ModelLabel, Section, Sheet, Skeleton, Text, icons } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { api, data, ifMatch } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { CatalogueModelView, Fields } from "~/api/generated/types";
import { aliasesQuery, catalogueModelQuery } from "~/api/queries/models";
import { playgroundModelsQuery } from "~/api/queries/playground";
import { projectCatalogue } from "~/api/queries/invalidation";
import { useApiMutation } from "~/auth/mutation";
import { moneyIn } from "~/format/currency";
import { type Prices } from "~/format/prices";
import { exchangeRatesQuery } from "~/api/queries/exchange-rates";
import { useOrgPrices } from "~/kaeso/org-prices";
import { dialectLabel } from "~/format/labels";
import { useProviders } from "~/kaeso/providers";
import { fullDayLabel } from "~/format/period";
import { usePartyName } from "../keys/party";
import { availabilityLabel, unavailableHint } from "./availability";
import { capabilities, decisionLabels, fieldRows, tierLabels, unitPriceLabels, type FieldKind } from "./model-fields";
import { ModelForm, changedFields, draftProblem, editBase, modalityLabel, startDraft, type FieldDraft } from "./model-form";
import { modelName } from "~/format/models";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { LinePanel } from "./lines";
import { useNeedsRight } from "~/auth/needs-right";
import { SheetLinks } from "~/kaeso/sheet-links";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { usePhone } from "~/shell/use-phone";

export function ModelSheet({ org, project, modelId, onClose }: { org: string; project: string; modelId: string | undefined; onClose: () => void }) {
  return (
    <Sheet
      open={!!modelId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={modelId ? <SheetTitle org={org} project={project} modelId={modelId} /> : ""}
    >
      {modelId && <ModelBody key={modelId} org={org} project={project} modelId={modelId} onClose={onClose} />}
    </Sheet>
  );
}

function SheetTitle({ org, project, modelId }: { org: string; project: string; modelId: string }) {
  const model = useQuery(catalogueModelQuery(org, project, modelId));
  useRememberRecent(
    model.data ? { kind: "model", id: modelId, label: modelName(model.data), href: `/orgs/${org}/projects/${project}/models?model=${modelId}` } : null,
  );
  return <>{model.data ? modelName(model.data) : common.model()}</>;
}

function ModelBody({ org, project, modelId, onClose }: { org: string; project: string; modelId: string; onClose: () => void }) {
  const needs = useNeedsRight(org, project);
  const rights = useProjectRights(org, project);
  const canManage = rights.has("models.manage");
  // Only a model the Playground can call gets a way there.
  const playground = useQuery({ ...playgroundModelsQuery(org, project), enabled: rights.has("playground.use") });
  const model = useQuery(catalogueModelQuery(org, project, modelId));
  const partyName = usePartyName(org, project);
  const providers = useProviders(org);
  const [editing, setEditing] = useState<FieldDraft | null>(null);
  const [start, setStart] = useState<FieldDraft | undefined>(undefined);
  const [removing, setRemoving] = useState(false);
  const prices = useOrgPrices(org);
  const rates = useQuery(exchangeRatesQuery);
  const phone = usePhone();

  const patch = useApiMutation({
    mutationFn: async (overrides: Partial<Fields> | Record<string, null>) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/models/{model}", {
          params: { path: { org, project, model: modelId }, header: ifMatch(model.data) },
          body: { overrides: overrides },
        }),
      ),
    invalidates: projectCatalogue(org, project),
    onError: (error) => {
      if (error.status === 412) setEditing(null);
    },
  });
  if (model.isPending) return <Skeleton className="w-full" />;
  if (model.isError) {
    return (
      <Empty
        title={
          isApiError(model.error) && model.error.type === "not_found"
            ? t("models.notFound", "This model doesn't exist or was removed.")
            : t("models.loadFailed", "Couldn't load this model.")
        }
        actions={<Button onClick={onClose}>{t("models.backToModels", "Back to models")}</Button>}
      />
    );
  }
  const m = model.data;
  const shown = inUse(m);
  const base = editBase(shown, m.overrides);
  const provider = providers.name(m.provider);
  const dialects = providers.get(m.provider) ? providers.dialects(m.provider) : shown.egress_dialect ? [shown.egress_dialect] : [];
  const reason = canManage ? undefined : needs("models.manage");

  if (editing) {
    const problem = draftProblem(editing, m.custom);
    const changes = changedFields(editing, base, dialects, start);
    const save = () =>
      patch.mutate(changes, {
        onSuccess: () => {
          toast(t("models.overridesSaved", "Overrides saved"));
          setEditing(null);
        },
      });
    return (
      <div className="flex flex-col gap-20">
        <ModelForm
          value={editing}
          onChange={setEditing}
          dialects={dialects}
          orgCurrency={prices.money.code}
          overrideNote={!providers.get(m.provider)?.custom}
        />
        <div className="flex justify-end gap-8">
          <Button
            onClick={() => {
              setEditing(null);
            }}
          >
            {common.cancel()}
          </Button>
          <Button
            variant="primary"
            loading={patch.isPending}
            disabled={!!problem || Object.keys(changes).length === 0}
            {...(problem ? { disabledReason: problem } : {})}
            onClick={save}
          >
            {t("models.saveOverrides", "Save overrides")}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-24">
      <div className="flex flex-wrap items-center justify-between gap-x-12 gap-y-4">
        <ModelLabel className="max-w-full shrink-0" provider={m.provider} providerName={provider} model={m.model_id} capabilities={capabilities(m.effective)} />
        <span className="flex items-center gap-4">
          {playground.data?.some((p) => p.id === m.id) && (
            <Button
              size="sm"
              variant="ghost"
              leadingIcon={icons.playground}
              href={`/orgs/${org}/projects/${project}/playground?models=${encodeURIComponent(`${m.provider}/${m.model_id}`)}`}
            >
              {t("models.tryInPlayground", "Try in Playground")}
            </Button>
          )}
          <Menu>
            <Menu.Trigger>
              <Button
                variant="ghost"
                size="sm"
                icon={icons.more}
                aria-label={t("models.actions", "Model actions")}
                disabled={!canManage}
                {...(reason ? { disabledReason: reason } : {})}
              />
            </Menu.Trigger>
            <Menu.Content align="end">
              <Menu.Item icon={icons.delete} variant="danger" onClick={() => setRemoving(true)}>
                {t("models.removeFromCatalogue", "Remove from catalogue")}
              </Menu.Item>
            </Menu.Content>
          </Menu>
        </span>
      </div>
      <Flags m={m} provider={provider} />
      <KeyValue
        items={[
          { label: common.provider(), value: provider },
          { label: t("models.modelId", "Model id"), value: m.model_id, mono: true, copy: m.model_id },
          { label: t("models.aliases", "Aliases"), value: m.aliases.length ? <AliasLinks org={org} project={project} names={m.aliases} /> : common.none() },
          { label: t("models.availability", "Availability"), value: availabilityLabel(m) },
          {
            label: t("models.addedAt", "Added"),
            value: t("models.addedBy", "{date} by {name}", { date: formatDate(m.created_at), name: partyName(m.created_by) }),
            wrap: true,
          },
        ]}
      />
      <SheetLinks org={org} project={project} requests={{ model: m.line_target ?? m.model_id, provider: m.provider }} target={m.id} />
      {m.line_target && <LinePanel org={org} project={project} id={m.id} canManage={canManage} reason={reason} />}
      <Section
        title={t("models.valuesInUse", "Values in use")}
        spacing="none"
        actions={
          <Button
            size="sm"
            variant="ghost"
            leadingIcon={icons.edit}
            disabled={!canManage}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => {
              const first = startDraft(base, prices.money.code, rates.data);
              setStart(first);
              setEditing(first);
            }}
          >
            {t("models.editOverrides", "Edit overrides")}
          </Button>
        }
      >
        <KeyValue
          layout={phone ? "stacked" : "columns"}
          items={fieldRows().map((row) => {
            const overridden = m.overrides[row.key] !== null && m.overrides[row.key] !== undefined;
            const value = shown[row.key];
            const tiers =
              row.kind === "tiers"
                ? tierLabels(shown, prices)
                : row.kind === "units"
                  ? unitPriceLabels(shown, prices)
                  : row.kind === "decisions" && shown.decisions
                    ? decisionLabels(shown.decisions)
                    : [];
            return {
              label: row.label,
              value: (
                <span className="flex min-w-0 items-center gap-8">
                  {tiers.length ? (
                    <span className="flex min-w-0 flex-col">
                      {tiers.map((label) => (
                        <Text key={label} tone={overridden ? "default" : "secondary"} truncate>
                          {label}
                        </Text>
                      ))}
                    </span>
                  ) : (
                    <Text tone={overridden ? "default" : "secondary"} truncate>
                      {row.kind === "price" && overridden && m.overrides.currency
                        ? entered(prices, m.overrides[row.key] as string, m.overrides.currency, value as string | null | undefined)
                        : show(prices, row.kind, row.key, value)}
                    </Text>
                  )}
                  <Text variant="caption" tone="tertiary">
                    {sourceOf(m, overridden, row.key, value)}
                  </Text>
                  {overridden && !m.custom && canManage && (
                    <Button
                      variant="ghost"
                      size="xs"
                      loading={patch.isPending && patch.variables?.[row.key] === null}
                      onClick={() =>
                        patch.mutate({ [row.key]: null }, { onSuccess: () => toast(t("models.fieldReset", "{field} reset to KPMR", { field: row.label })) })
                      }
                    >
                      {t("models.resetToKpmr", "Reset to KPMR")}
                    </Button>
                  )}
                </span>
              ),
            };
          })}
        />
      </Section>
      <DialogScope open={removing}>
        <RemoveModelDialog org={org} project={project} model={m} onRemoved={onClose} open={removing} onOpenChange={setRemoving} />
      </DialogScope>
    </div>
  );
}

// Each alias opens its route editor; one the list doesn't have yet stays text.
function AliasLinks({ org, project, names }: { org: string; project: string; names: string[] }) {
  const aliases = useQuery(aliasesQuery(org, project));
  return names.map((name, i) => {
    const id = aliases.data?.find((a) => a.name === name)?.id;
    return (
      <Fragment key={name}>
        {i > 0 && ", "}
        {id ? <Link href={`/orgs/${org}/projects/${project}/models/aliases/${id}`}>{name}</Link> : name}
      </Fragment>
    );
  });
}

function RemoveModelDialog({
  org,
  project,
  model,
  onRemoved,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  model: CatalogueModelView;
  onRemoved: () => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/models/{model}", {
        params: { path: { org, project, model: model.id }, header: ifMatch(model) },
      });
    },
    invalidates: projectCatalogue(org, project),
    onSuccess: () => {
      toast(t("models.removed", "Model removed from the catalogue"));
      onRemoved();
    },
    onError: (error) => {
      if (error.status === 412) onOpenChange(false);
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("models.removeTitle", "Remove {model}?", { model: modelName(model) })}
      description={t("models.removeHint", "Requests naming it are refused, and aliases and model lists that name it lose it.")}
      confirmLabel={t("models.removeFromCatalogue", "Remove from catalogue")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}

function Flags({ m, provider }: { m: CatalogueModelView; provider: string }) {
  const count = Object.values(m.overrides).filter((v) => v !== null && v !== undefined).length;
  const flags: ReactNode[] = [];
  const hints: ReactNode[] = [];
  if (m.availability === "unavailable") {
    flags.push(
      <Badge key="unavailable" tone="danger">
        {t("models.unavailable", "Unavailable")}
      </Badge>,
    );
    hints.push(unavailableHint(provider));
  }
  if (m.deprecated) {
    flags.push(
      <Badge key="deprecated" tone="warning">
        {t("models.deprecated", "Deprecated")}
      </Badge>,
    );
    hints.push(t("models.deprecatedHint", "KPMR marks it deprecated; plan a move to a newer model."));
  }
  if (m.custom) flags.push(<Badge key="custom">{t("models.custom", "Custom")}</Badge>);
  else if (count) flags.push(<Badge key="over">{t("models.overriddenCount", "Overridden {count}", { count })}</Badge>);
  if (!m.listed && !m.custom)
    flags.push(
      <Badge key="delisted" tone="warning">
        {m.delisted_at
          ? t("models.delistedSince", "No longer listed since {date}", { date: formatDate(m.delisted_at) })
          : t("models.delisted", "No longer listed")}
      </Badge>,
    );
  if (!m.listed && !m.custom) hints.push(t("models.delistedHint", "KPMR stopped listing it; it keeps its last known values."));
  if (!flags.length) return null;
  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-wrap items-center gap-8">{flags}</div>
      {hints.map((hint, i) => (
        <Text key={i} variant="small" tone="secondary">
          {hint}
        </Text>
      ))}
    </div>
  );
}

function sourceOf(m: CatalogueModelView, overridden: boolean, key: keyof Fields, value: unknown): string {
  if (overridden) return m.custom ? t("models.source.setHere", "set here") : t("models.source.override", "override");
  if (value === null || value === undefined || (Array.isArray(value) && value.length === 0)) return "";
  if (key === "egress_dialect" && !m.effective.egress_dialect) return "Kaeso";
  return "KPMR";
}

function inUse(m: Pick<CatalogueModelView, "effective" | "egress_dialect">): Fields {
  return { ...m.effective, egress_dialect: m.effective.egress_dialect ?? m.egress_dialect };
}

// A price entered in the organization's currency reads as entered; one in another currency is converted like every other.
function entered(prices: Prices, price: string, code: string, usd: string | null | undefined): string {
  if (!usd) return t("models.pricePerMillionNoRate", "{price} / M tokens (no exchange rate)", { price: moneyIn(code).price(price) });
  return t("models.pricePerMillion", "{price} / M tokens", { price: code === prices.money.code ? prices.money.price(price) : prices.unit(usd) });
}

function show(prices: Prices, kind: FieldKind, key: keyof Fields, v: unknown): string {
  if (v === null || v === undefined || (Array.isArray(v) && v.length === 0)) return "—";
  switch (kind) {
    case "price":
      return t("models.pricePerMillion", "{price} / M tokens", { price: prices.unit(String(v)) });
    case "number":
      return key === "timeout_seconds"
        ? t("models.seconds", "{seconds} s", { seconds: formatNumber(Number(v)) })
        : t("models.tokens", "{tokens} tokens", { tokens: formatNumber(Number(v)) });
    case "bool":
      return v ? common.yes() : common.no();
    case "list":
      return (v as string[]).map((x) => (key === "efforts" ? x : modalityLabel(x))).join(t("models.listSeparator", ", "));
    case "dialect":
      return dialectLabel(String(v));
    case "date":
      return fullDayLabel(String(v));
    default:
      return String(v);
  }
}
