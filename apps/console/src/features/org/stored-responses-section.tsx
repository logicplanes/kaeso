import { useQuery } from "@tanstack/react-query";
import { SettingsGroup, SettingsRow, Section, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { orgStoredResponsesQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

export function OrgStoredResponsesSection({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const canUpdate = useOrgRights(org).has("org.update");
  const policy = useQuery(orgStoredResponsesQuery(org));
  const save = useApiMutation({
    mutationFn: async (allowed: boolean) => data(await api.PATCH("/platform/v1/orgs/{org}/stored-responses", { params: { path: { org } }, body: { allowed } })),
    invalidates: [
      ["orgs", org, "stored-responses"],
      ["orgs", org, "projects"],
    ],
    onSuccess: (p) =>
      toast.success(
        p.allowed ? t("org.storedResponses.allowedToast", "Stored responses allowed") : t("org.storedResponses.forbiddenToast", "Stored responses turned off"),
      ),
  });
  const p = policy.data;
  const reason = !canUpdate ? needs("org.update") : undefined;
  return (
    <Section
      spacing="settings"
      title={t("org.storedResponses.title", "Stored responses")}
      description={t("org.storedResponses.hint", "Providers such as OpenAI may keep responses so apps can continue them.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.storedResponses.allow", "Allow responses stored at the provider")}
          description={
            p?.allowed
              ? t("org.storedResponses.allowedHint", "Each project turns it on in its settings.")
              : t("org.storedResponses.forbiddenHint", "Apps must send each conversation whole.")
          }
          control={
            <Switch
              aria-label={t("org.storedResponses.allow", "Allow responses stored at the provider")}
              checked={!!p?.allowed}
              disabled={!p || !!reason}
              {...(reason ? { disabledReason: reason } : {})}
              loading={save.isPending}
              onCheckedChange={(on) => save.mutate(on)}
            />
          }
        />
      </SettingsGroup>
    </Section>
  );
}
