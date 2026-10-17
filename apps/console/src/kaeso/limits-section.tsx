import { useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { AlertDialog, Button, Dialog, IconButton, LimitMeter, Section, Text, icons } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatNumber, formatRelative, formatUSD } from "@logicplanes/kaeso-ui/format";
import { api, data, ifMatch } from "~/api/client";
import type { LimitView } from "~/api/generated/types";
import { limitsQuery, type LimitFilter } from "~/api/queries/limits";
import { catalogueQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { limitDescription, limitKindLabel, periodLabel } from "~/format/labels";
import { alertMarks } from "~/format/alert-marks";
import { modelName } from "~/format/models";
import { t } from "~/i18n";
import { parentConflict, parentFilters } from "./limit-checks";
import { LimitEditor, alertThresholds, limitBody, limitProblem, newLimit, type LimitDraft } from "./limit-editor";

export type LimitOwner = { kind: "user" | "service_account"; id: string };

export function useParentLimits(org: string, project: string, target: { level: "project" | "member" | "key" }, owner?: LimitOwner): LimitView[] {
  const results = useQueries({ queries: parentFilters(target, owner).map((f) => limitsQuery(org, project, f)) });
  return results.flatMap((r) => r.data ?? []);
}

export function limitBlocker(draft: LimitDraft, parents: LimitView[]): string | null {
  if (!draft.amount) return t("kaeso.limits.enterAmount", "Enter an amount");
  const own = limitProblem(draft);
  if (own) return own;
  const over = parentConflict(draft, parents);
  return over
    ? t(
        "kaeso.limits.higherThanAbove",
        "Higher than the {level, select, project {project's} other {member's}} {limit}; a lower level can't be higher than the one above it",
        { level: over.level, limit: limitDescription(over) },
      )
    : null;
}

export type LimitTarget =
  { level: "project" } | { level: "member"; user: string } | { level: "member"; service_account: string } | { level: "key"; key: string };

export function LimitsSection({
  org,
  project,
  filter,
  target,
  canEdit,
  empty,
  owner,
}: {
  org: string;
  project: string;
  filter: LimitFilter;
  target: LimitTarget;
  canEdit: boolean;
  empty: string;
  owner?: LimitOwner | undefined;
}) {
  const limits = useQuery(limitsQuery(org, project, filter));
  const parents = useParentLimits(org, project, target, owner);
  const catalogue = useQuery(catalogueQuery(org, project));
  const [draft, setDraft] = useState<LimitDraft | null>(null);
  const blocker = draft && limitBlocker(draft, parents);
  const add = useApiMutation({
    mutationFn: async (d: LimitDraft) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/limits", {
          params: { path: { org, project } },
          body: { ...target, ...limitBody(d) },
        }),
      ),
    invalidates: [["orgs", org, "projects", project, "limits"]],
    onSuccess: () => {
      setDraft(null);
      toast.success(t("kaeso.limits.added", "Limit added"));
    },
  });
  return (
    <Section
      title={t("kaeso.limits.title", "Limits")}
      spacing="none"
      actions={
        canEdit && !draft ? (
          <Button size="sm" variant="ghost" leadingIcon={icons.add} onClick={() => setDraft(newLimit())}>
            {t("kaeso.limits.add", "Add limit")}
          </Button>
        ) : undefined
      }
    >
      <div className="flex flex-col gap-16">
        {(limits.data ?? []).length === 0 && !draft && <Text tone="secondary">{empty}</Text>}
        {(limits.data ?? []).map((l) => (
          <LimitRow key={l.id} org={org} project={project} limit={l} canEdit={canEdit} />
        ))}
        {draft && (
          <div className="flex flex-col gap-12">
            <LimitEditor value={draft} onChange={setDraft} models={catalogue.data ?? []} />
            <div className="flex gap-8">
              <Button
                size="sm"
                variant="primary"
                disabled={!!blocker}
                {...(blocker ? { disabledReason: blocker } : {})}
                loading={add.isPending}
                onClick={() => add.mutate(draft)}
              >
                {t("kaeso.limits.add", "Add limit")}
              </Button>
              <Button size="sm" onClick={() => setDraft(null)}>
                {t("common.cancel", "Cancel")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </Section>
  );
}

export function useLimitText(org: string, project: string, limit: LimitView) {
  const catalogue = useQuery(catalogueQuery(org, project));
  const money = limit.kind === "spend_budget";
  const fmt = (v: string) => (money ? formatUSD(v) : formatNumber(Number(v)));
  const catalogueModel = limit.model ? catalogue.data?.find((m) => m.id === limit.model) : undefined;
  return {
    kind: (money ? "spend" : limit.kind === "request_rate" ? "requests" : "tokens") as "spend" | "requests" | "tokens",
    value: t("kaeso.limits.usedOf", "{used} of {amount}", { used: fmt(limit.used ?? "0"), amount: fmt(limit.amount) }),
    model: limit.model ? (catalogueModel ? modelName(catalogueModel) : limit.model) : t("kaeso.limits.allModels", "All models"),
    resets:
      limit.period === "minute"
        ? t("kaeso.limits.overLastMinute", "Over the last minute")
        : limit.resets_at
          ? t("kaeso.limits.resets", "Resets {when}", { when: formatRelative(limit.resets_at) })
          : t("kaeso.limits.neverResets", "Never resets"),
  };
}

function LimitRow({ org, project, limit, canEdit }: { org: string; project: string; limit: LimitView; canEdit: boolean }) {
  const [deleting, setDeleting] = useState(false);
  const [editing, setEditing] = useState(false);
  const text = useLimitText(org, project, limit);
  return (
    <div className="flex items-start gap-12">
      <div className="min-w-0 flex-1">
        <LimitMeter
          kind={text.kind}
          scope={limitKindLabel(limit.kind)}
          window={`${periodLabel(limit.period)} · ${text.model}`}
          used={Number(limit.used ?? 0)}
          limit={Number(limit.amount)}
          valueLabel={text.value}
          alerts={alertMarks(limit)}
          hard={limit.hard}
          resets={text.resets}
        />
      </div>
      {canEdit && (
        <span className="flex items-center gap-4">
          <IconButton
            icon={icons.edit}
            aria-label={t("kaeso.limits.editThe", "Edit the {limit}", { limit: limitDescription(limit) })}
            size="sm"
            onClick={() => setEditing(true)}
          />
          <IconButton
            icon={icons.delete}
            aria-label={t("kaeso.limits.removeThe", "Remove the {limit}", { limit: limitDescription(limit) })}
            size="sm"
            onClick={() => setDeleting(true)}
          />
        </span>
      )}
      {editing && <EditLimitDialog org={org} project={project} limit={limit} onClose={() => setEditing(false)} />}
      <RemoveLimitDialog org={org} project={project} limit={limit} open={deleting} onOpenChange={setDeleting} />
    </div>
  );
}

export function EditLimitDialog({
  org,
  project,
  limit,
  onClose,
  onRemove,
}: {
  org: string;
  project: string;
  limit: LimitView;
  onClose: () => void;
  onRemove?: (() => void) | undefined;
}) {
  const catalogue = useQuery(catalogueQuery(org, project));
  const [editing, setEditing] = useState<LimitDraft>(() => ({
    kind: limit.kind as LimitDraft["kind"],
    amount: limit.amount,
    period: limit.period as LimitDraft["period"],
    model: limit.model ?? null,
    hard: limit.hard,
    alert_thresholds: limit.alert_thresholds,
  }));
  const update = useApiMutation({
    mutationFn: async (d: LimitDraft) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/limits/{limit}", {
          params: { path: { org, project, limit: limit.id }, header: ifMatch(limit) },
          body: { amount: d.amount, alert_thresholds: alertThresholds(d), ...(d.kind === "token_budget" || d.kind === "spend_budget" ? { hard: d.hard } : {}) },
        }),
      ),
    invalidates: [["orgs", org, "projects", project, "limits"]],
    onSuccess: () => {
      onClose();
      toast.success(t("kaeso.limits.saved", "Limit saved"));
    },
    onError: (error) => {
      if (error.status === 412) onClose();
    },
  });
  const problem = limitProblem(editing);
  return (
    <Dialog
      busy={update.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("kaeso.limits.edit", "Edit limit")}
      description={t("kaeso.limits.editHint", "The {limit}; to change its kind, period or model, remove it and add another.", {
        limit: limitDescription(limit),
      })}
      onSubmit={() => !problem && update.mutate(editing)}
      footer={
        <>
          {onRemove && (
            <Button variant="danger-secondary" className="mr-auto" disabled={update.isPending} onClick={onRemove}>
              {t("kaeso.limits.remove", "Remove limit")}
            </Button>
          )}
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            disabled={!!problem}
            {...(problem ? { disabledReason: problem } : {})}
            loading={update.isPending}
            onClick={() => update.mutate(editing)}
          >
            {t("kaeso.limits.save", "Save limit")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        <LimitEditor value={editing} onChange={setEditing} models={catalogue.data ?? []} fixedKind />
      </div>
    </Dialog>
  );
}

export function RemoveLimitDialog({
  org,
  project,
  limit,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  limit: LimitView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/limits/{limit}", {
        params: { path: { org, project, limit: limit.id }, header: ifMatch(limit) },
      });
    },
    invalidates: [["orgs", org, "projects", project, "limits"]],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("kaeso.limits.removed", "Limit removed"));
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
      title={t("kaeso.limits.removeTitle", "Remove this limit?")}
      description={t("kaeso.limits.removeHint", "The {limit} stops applying; the levels above it still do.", { limit: limitDescription(limit) })}
      confirmLabel={t("kaeso.limits.remove", "Remove limit")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
