import { Section, Select, SettingsGroup, SettingsRow } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SharedLinksPolicy as Policy } from "~/api/generated/types";
import { securityQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { expiryLabel } from "~/kaeso/share-button";
import { toast } from "~/shell/toast";

const NEVER = "never";

export function SharedLinksPolicy({ org, policy }: { org: string; policy: Policy }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const canManage = rights.has("usage.share.manage");
  const missing = canManage ? undefined : needs("usage.share.manage");
  const save = useApiMutation({
    mutationFn: async (next: Policy) => data(await api.PUT("/platform/v1/orgs/{org}/security/shared-links", { params: { path: { org } }, body: next })),
    invalidates: [securityQuery(org).queryKey],
    onSuccess: () => toast.success(t("org.sharedPolicy.saved", "Shared links saved")),
  });
  return (
    <Section
      spacing="settings"
      title={t("org.sharedPolicy.title", "Shared links")}
      description={t("org.sharedPolicy.description", "Read-only links to usage, member stats and budgets; never to request logs.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.sharedPolicy.allow", "Allow shared links")}
          description={missing ?? t("org.sharedPolicy.allowHint", "Tightening it applies to links made before, too.")}
          control={
            <Select
              aria-label={t("org.sharedPolicy.allow", "Allow shared links")}
              size="sm"
              value={policy.allow}
              disabled={!canManage || save.isPending}
              onValueChange={(allow) => save.mutate({ ...policy, allow })}
              options={[
                { value: "off", label: t("org.sharedPolicy.off", "Off") },
                { value: "members", label: t("org.sharedPolicy.members", "Members only") },
                { value: "anyone", label: t("org.sharedPolicy.anyone", "Anyone with the link") },
              ]}
            />
          }
        />
        <SettingsRow
          title={t("org.sharedPolicy.maxExpiry", "Longest expiry")}
          description={t("org.sharedPolicy.maxExpiryHint", "Links made before keep their expiry.")}
          control={
            <Select
              aria-label={t("org.sharedPolicy.maxExpiry", "Longest expiry")}
              size="sm"
              value={policy.max_days == null ? NEVER : String(policy.max_days)}
              disabled={!canManage || save.isPending}
              onValueChange={(v) => save.mutate({ ...policy, max_days: v === NEVER ? null : Number(v) })}
              options={[1, 7, 30, 90, NEVER].map((d) => ({
                value: String(d),
                label: d === NEVER ? t("org.sharedPolicy.noLimit", "No limit") : expiryLabel(d as number),
              }))}
            />
          }
        />
      </SettingsGroup>
    </Section>
  );
}
