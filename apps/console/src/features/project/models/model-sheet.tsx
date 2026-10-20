import { useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Button, Empty, KeyValue, Menu, IconButton, ModelLabel, Sheet, Skeleton, Text, icons } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate, formatNumber, formatUSD } from "@logicplanes/kaeso-ui/format";
import { api, data, ifMatch } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { CatalogueModelView, Fields } from "~/api/generated/types";
import { catalogueModelQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { dialectLabel } from "~/format/labels";
import { useProviders } from "~/kaeso/providers";
import { fullDayLabel } from "~/format/period";
import { usePartyName } from "../keys/party";
import { availabilityLabel, unavailableHint } from "./availability";
import { capabilities, fieldRows, type FieldKind } from "./model-fields";
import { ModelForm, changedFields, draftFrom, draftProblem, modalityLabel, type FieldDraft } from "./model-form";
import { modelName } from "~/format/models";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const invalidates = (org: string, project: string) => [
  ["orgs", org, "projects", project, "models"],
  ["orgs", org, "projects", project, "aliases"],
];

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
  const model = useQuery(catalogueModelQuery(org, project, modelId));
  const partyName = usePartyName(org, project);
  const providers = useProviders(org);
  const [editing, setEditing] = useState<FieldDraft | null>(null);
  const [removing, setRemoving] = useState(false);

  const patch = useApiMutation({
    mutationFn: async (overrides: Partial<Fields> | Record<string, null>) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/models/{model}", {
          params: { path: { org, project, model: modelId }, header: ifMatch(model.data) },
          body: { overrides: overrides },
        }),
      ),
    invalidates: invalidates(org, project),
    onError: (error) => {
      if (error.status === 412) setEditing(null);
    },
  });
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/models/{model}", {
        params: { path: { org, project, model: modelId }, header: ifMatch(model.data) },
      });
    },
    invalidates: invalidates(org, project),
    onSuccess: () => {
      toast(t("models.removed", "Model removed from the catalogue"));
      onClose();
    },
    onError: (error) => {
      if (error.status === 412) setRemoving(false);
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
  const provider = providers.name(m.provider);
  const dialects = providers.dialects(m.provider);
  const reason = canManage ? undefined : needs("models.manage");

  if (editing) {
    const problem = draftProblem(editing, m.custom);
    const changes = changedFields(editing, shown, dialects);
    const save = () =>
      patch.mutate(changes, {
        onSuccess: () => {
          toast(t("models.overridesSaved", "Overrides saved"));
          setEditing(null);
        },
      });
    return (
      <div className="flex flex-col gap-20">
        <ModelForm value={editing} onChange={setEditing} dialects={dialects} />
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
      <div className="flex items-center justify-between gap-12">
        <ModelLabel
          provider={m.provider}
          providerName={provider}
          model={m.model_id}
          capabilities={capabilities(m.effective)}
          {...(m.effective.display_name ? { name: m.effective.display_name } : {})}
        />
        <Menu>
          <Menu.Trigger>
            <IconButton
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
      </div>
      <Flags m={m} provider={provider} />
      <KeyValue
        items={[
          { label: common.provider(), value: provider },
          { label: t("models.modelId", "Model id"), value: m.model_id, mono: true, copy: m.model_id },
          { label: t("models.aliases", "Aliases"), value: m.aliases.length ? m.aliases.join(", ") : common.none() },
          { label: t("models.availability", "Availability"), value: availabilityLabel(m) },
          {
            label: t("models.addedAt", "Added"),
            value: t("models.addedBy", "{date} by {name}", { date: formatDate(m.created_at), name: partyName(m.created_by) }),
          },
        ]}
      />
      <div className="flex flex-col gap-12">
        <div className="flex items-center justify-between gap-12">
          <Text weight="medium">{t("models.valuesInUse", "Values in use")}</Text>
          <Button
            size="sm"
            leadingIcon={icons.edit}
            disabled={!canManage}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => setEditing(draftFrom(shown))}
          >
            {t("models.editOverrides", "Edit overrides")}
          </Button>
        </div>
        <KeyValue
          items={fieldRows().map((row) => {
            const overridden = m.overrides[row.key] !== null && m.overrides[row.key] !== undefined;
            const value = shown[row.key];
            return {
              label: row.label,
              value: (
                <span className="flex min-w-0 items-center gap-8">
                  <Text tone={overridden ? "default" : "secondary"} truncate>
                    {show(row.kind, row.key, value)}
                  </Text>
                  <Text variant="caption" tone="tertiary">
                    {sourceOf(m, overridden, row.key, value)}
                  </Text>
                  {overridden && !m.custom && canManage && (
                    <Button
                      variant="ghost"
                      size="xs"
                      loading={patch.isPending && patch.variables?.[row.key] === null}
                      onClick={() =>
                        patch.mutate(
                          { [row.key]: null },
                          { onSuccess: () => toast(t("models.fieldReset", "{field} reset to models.dev", { field: row.label })) },
                        )
                      }
                    >
                      {t("models.resetToModelsDev", "Reset to models.dev")}
                    </Button>
                  )}
                </span>
              ),
            };
          })}
        />
      </div>
      <AlertDialog
        open={removing}
        onOpenChange={setRemoving}
        tone="danger"
        title={t("models.removeTitle", "Remove {model}?", { model: modelName(m) })}
        description={t("models.removeHint", "Requests naming it are refused, and aliases and model lists that name it lose it.")}
        confirmLabel={t("models.removeFromCatalogue", "Remove from catalogue")}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync()}
      />
    </div>
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
    hints.push(t("models.deprecatedHint", "models.dev marks it deprecated; plan a move to a newer model."));
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
  if (!m.listed && !m.custom) hints.push(t("models.delistedHint", "models.dev stopped listing it; it keeps its last known values."));
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
  return "models.dev";
}

function inUse(m: Pick<CatalogueModelView, "effective" | "egress_dialect">): Fields {
  return { ...m.effective, egress_dialect: m.effective.egress_dialect ?? m.egress_dialect };
}

function show(kind: FieldKind, key: keyof Fields, v: unknown): string {
  if (v === null || v === undefined || (Array.isArray(v) && v.length === 0)) return "—";
  switch (kind) {
    case "price":
      return t("models.pricePerMillion", "{price} / M tokens", { price: formatUSD(String(v)) });
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
