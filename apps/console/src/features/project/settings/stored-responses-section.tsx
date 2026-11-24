import { useQuery } from "@tanstack/react-query";
import { SettingsGroup, SettingsRow, Section, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { projectStoredResponsesQuery } from "~/api/queries/project-settings";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

export function StoredResponsesSection({ org, project, canChange, archived }: { org: string; project: string; canChange: boolean; archived: boolean }) {
  const needs = useNeedsRight(org, project);
  const query = projectStoredResponsesQuery(org, project);
  const settings = useQuery(query);
  const save = useApiMutation({
    mutationFn: async (enabled: boolean) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/stored-responses", { params: { path: { org, project } }, body: { enabled } })),
    invalidates: [query.queryKey],
    onSuccess: (s) =>
      toast.success(
        s.enabled ? t("project.storedResponses.on", "Stored responses turned on") : t("project.storedResponses.off", "Stored responses turned off"),
      ),
  });
  const s = settings.data;
  const locked = !canChange
    ? needs("project.update")
    : archived
      ? t("project.settings.archivedReadOnly", "An archived project can't change")
      : s && !s.enabled && !s.org_allowed
        ? t("project.storedResponses.orgOff", "Off for the organization")
        : undefined;
  return (
    <Section
      spacing="settings"
      title={t("project.storedResponses.title", "Stored responses")}
      description={t("project.storedResponses.hint", "Lets Responses apps continue a response that OpenAI keeps for 30 days.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("project.storedResponses.use", "Allow responses stored at the provider")}
          description={
            !s
              ? undefined
              : !s.org_allowed
                ? t("project.storedResponses.orgOffHint", "Off for the organization; it can't be turned on here.")
                : s.enabled
                  ? t("project.storedResponses.onHint", "Apps may use store and previous_response_id.")
                  : t("project.storedResponses.offHint", "Apps must send each conversation whole.")
          }
          control={
            <Switch
              aria-label={t("project.storedResponses.use", "Allow responses stored at the provider")}
              checked={!!s?.enabled}
              disabled={!s || !!locked}
              {...(locked ? { disabledReason: locked } : {})}
              loading={save.isPending}
              onCheckedChange={(on) => save.mutate(on)}
            />
          }
        />
      </SettingsGroup>
    </Section>
  );
}
