import { Select, SettingsGroup, SettingsRow, Section } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { projectListKey, projectViewKey } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

/** How many times a step tries its model again, as people read it: never, once, twice or 3 times. */
export const retriesLabel = (count: number): string =>
  t("project.retries.count", "{count, plural, =0 {Never} one {Once} =2 {Twice} other {# times}}", { count });

export function RetriesSection({
  org,
  project,
  retries,
  canChange,
  archived,
}: {
  org: string;
  project: string;
  retries: number;
  canChange: boolean;
  archived: boolean;
}) {
  const save = useApiMutation({
    mutationFn: async (next: number) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}", { params: { path: { org, project } }, body: { retries: next } })),
    invalidates: [projectListKey(org), projectViewKey(org, project), ["orgs", org, "projects", project, "models"]],
    onSuccess: (p) => toast.success(t("project.retries.saved", "Tries again: {times}", { times: retriesLabel(p.retries).toLowerCase() })),
  });
  // Read-only without Edit project or in an archived project, as the rest of the page.
  const locked = !canChange || archived;
  return (
    <Section
      spacing="settings"
      title={t("project.retries.title", "Retries")}
      description={t("project.retries.hint", "Tries a model again when its provider fails for a moment.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("project.retries.tryAgain", "Try again")}
          description={t("project.retries.tryAgainHint", "After a dropped connection, a 5xx or an overload.")}
          control={
            <div className="w-160">
              <Select
                aria-label={t("project.retries.tryAgain", "Try again")}
                size="sm"
                fullWidth
                value={String(retries)}
                disabled={locked || save.isPending}
                onValueChange={(v) => save.mutate(Number(v))}
                options={[0, 1, 2, 3].map((n) => ({ value: String(n), label: retriesLabel(n) }))}
              />
            </div>
          }
        />
      </SettingsGroup>
    </Section>
  );
}
