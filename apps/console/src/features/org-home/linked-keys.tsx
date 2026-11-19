import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Badge, Button, Dialog, Select, Text, Tooltip } from "@logicplanes/kaeso-ui";
import { formatDate, formatUSD } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { CredentialView, LinkedBillingOfferView } from "~/api/generated/types";
import { linkedFallbacksQuery } from "~/api/queries/credentials";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { keyTitle, useProviders } from "~/kaeso/providers";
import { toast } from "~/shell/toast";

export const links = (k: CredentialView) => k.auth_method === "linked_billing";

// `offer` is missing when a custom provider hides the built-in provider the key was linked at.
export function LinkedStatus({ k, offer }: { k: CredentialView; offer: LinkedBillingOfferView | null | undefined }) {
  if (k.auth_error === "unlinked")
    return (
      <Tooltip content={t("orgHome.linkedUnlinkedHint", "The link ended; link it again to pay through it.")}>
        <Badge tone="warning">
          {offer ? t("orgHome.linkedUnlinked", "Unlinked in {console}", { console: offer.console }) : t("orgHome.unlinked", "Unlinked")}
        </Badge>
      </Tooltip>
    );
  if (k.auth_error === "needs_setup")
    return (
      <Tooltip content={t("orgHome.linkedNeedsSetupHint", "The install's OAuth app is missing or changed; an instance admin sets it up.")}>
        <Badge tone="warning">{t("orgHome.linkedNeedsSetup", "Needs instance setup")}</Badge>
      </Tooltip>
    );
  if (k.spend_limited_until)
    return (
      <Tooltip content={t("orgHome.spendLimitHint", "Until {date} at the latest, or until its admins raise it.", { date: formatDate(k.spend_limited_until) })}>
        <Badge tone="warning">{t("orgHome.spendLimit", "Spend limit reached")}</Badge>
      </Tooltip>
    );
  return null;
}

export function linkedCost(cost: string | undefined): string | null {
  return cost === undefined ? null : t("orgHome.linkedThisMonth", "{cost} this month", { cost: formatUSD(cost) });
}

const NONE = "none";

export function FallbackDialog({ org, credential, keys, onClose }: { org: string; credential: CredentialView; keys: CredentialView[]; onClose: () => void }) {
  const providers = useProviders(org);
  const projects = useQuery(orgProjectsQuery(org));
  const fallbacks = useQuery(linkedFallbacksQuery(org));
  const candidates = keys.filter((k) => k.provider === credential.provider && !links(k));
  const [project, setProject] = useState<string | null>(null);
  const chosenProject = project ?? projects.data?.[0]?.id ?? null;
  const current = fallbacks.data?.find((f) => f.project === chosenProject && f.provider === credential.provider)?.credential ?? NONE;
  const [fallback, setFallback] = useState<string | null>(null);
  const chosen = fallback ?? current;
  const save = useApiMutation({
    mutationFn: async (project: string) =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/linked-billing/fallbacks", {
          params: { path: { org } },
          body: { project, provider: credential.provider, credential: chosen === NONE ? null : chosen },
        }),
      ),
    invalidates: [["orgs", org, "credentials", "fallbacks"]],
    onSuccess: () => {
      toast(t("orgHome.fallbackSaved", "Fallback saved"));
      onClose();
    },
  });
  const submit = () => {
    if (chosenProject) save.mutate(chosenProject);
  };
  return (
    <Dialog
      busy={save.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.fallbackTitle", "Who pays when {title} can't", { title: keyTitle(credential, providers) })}
      description={t("orgHome.fallbackHint", "When its spend limit is reached or its link is gone, a project can use another key.")}
      onSubmit={submit}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            loading={save.isPending}
            disabled={!chosenProject}
            disabledReason={!chosenProject ? t("orgHome.fallbackProjectBlocked", "Choose a project") : undefined}
            onClick={submit}
          >
            {t("common.save", "Save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-12">
        <Select
          label={t("common.project", "Project")}
          value={chosenProject ?? ""}
          onValueChange={(v) => {
            setProject(v);
            setFallback(null);
          }}
          options={(projects.data ?? []).map((p) => ({ value: p.id, label: p.name }))}
        />
        <Select
          label={t("orgHome.fallbackKey", "When it can't pay")}
          value={chosen}
          onValueChange={setFallback}
          options={[
            { value: NONE, label: t("orgHome.fallbackNone", "The request fails") },
            ...candidates.map((k) => ({ value: k.id, label: t("orgHome.fallbackTo", "Fall back to {name}", { name: k.name }) })),
          ]}
        />
        {candidates.length === 0 && (
          <Text variant="small" tone="secondary">
            {t("orgHome.fallbackNoKeys", "Add an API key for {provider} to fall back to.", { provider: providers.name(credential.provider) })}
          </Text>
        )}
      </div>
    </Dialog>
  );
}
