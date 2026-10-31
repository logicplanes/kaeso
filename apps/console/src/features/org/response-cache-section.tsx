import { useQuery } from "@tanstack/react-query";
import { SettingsGroup, SettingsRow, SettingsSection, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { OrgResponseCache } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const orgResponseCacheQuery = (org: string) => ({
  queryKey: ["orgs", org, "response-cache"] as const,
  queryFn: async (): Promise<OrgResponseCache> => data(await api.GET("/platform/v1/orgs/{org}/response-cache", { params: { path: { org } } })),
});

export function OrgResponseCacheSection({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const canUpdate = useOrgRights(org).has("org.update");
  const policy = useQuery(orgResponseCacheQuery(org));
  const save = useApiMutation({
    mutationFn: async (allowed: boolean) => data(await api.PATCH("/platform/v1/orgs/{org}/response-cache", { params: { path: { org } }, body: { allowed } })),
    invalidates: [
      ["orgs", org, "response-cache"],
      ["orgs", org, "projects"],
    ],
    onSuccess: (p) =>
      toast.success(
        p.allowed ? t("org.responseCache.allowedToast", "Response cache allowed") : t("org.responseCache.forbiddenToast", "Response cache turned off"),
      ),
  });
  const p = policy.data;
  const unavailable = !!p && !p.available;
  const reason = !canUpdate ? needs("org.update") : unavailable ? t("org.responseCache.unavailable", "Off on this install") : undefined;
  return (
    <SettingsSection
      title={t("org.responseCache.title", "Response cache")}
      description={t("org.responseCache.hint", "Projects may answer a repeated identical request from a stored reply.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.responseCache.allow", "Allow the response cache")}
          description={
            unavailable
              ? t("org.responseCache.unavailableHint", "This install doesn't offer it.")
              : p?.allowed
                ? t("org.responseCache.allowedHint", "Each project turns it on in its settings.")
                : t("org.responseCache.forbiddenHint", "No project can use it, and stored replies are cleared.")
          }
          control={
            <Switch
              aria-label={t("org.responseCache.allow", "Allow the response cache")}
              checked={!!p?.allowed && !unavailable}
              disabled={!p || !!reason}
              {...(reason ? { disabledReason: reason } : {})}
              loading={save.isPending}
              onCheckedChange={(on) => save.mutate(on)}
            />
          }
        />
      </SettingsGroup>
    </SettingsSection>
  );
}
