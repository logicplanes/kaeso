import { useQuery } from "@tanstack/react-query";
import { Section, SettingsGroup, SettingsRow, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ModelLifecycleSettings } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const lifecycleQuery = (org: string, project: string) => ({
  queryKey: ["orgs", org, "projects", project, "model-lifecycle"] as const,
  queryFn: async (): Promise<ModelLifecycleSettings> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/model-lifecycle", { params: { path: { org, project } } })),
});

export function ModelLifecycleSection({ org, project, canChange, archived }: { org: string; project: string; canChange: boolean; archived: boolean }) {
  const needs = useNeedsRight(org, project);
  const query = lifecycleQuery(org, project);
  const settings = useQuery(query);
  const save = useApiMutation({
    mutationFn: async (body: ModelLifecycleSettings) =>
      data(await api.PUT("/platform/v1/orgs/{org}/projects/{project}/model-lifecycle", { params: { path: { org, project } }, body })),
    invalidates: [query.queryKey],
    onSuccess: () => toast.success(t("project.lifecycle.saved", "Model retirement settings saved")),
  });
  const s = settings.data;
  const locked = !canChange ? needs("models.manage") : archived ? t("project.settings.archivedReadOnly", "An archived project can't change") : undefined;
  const row = (key: keyof ModelLifecycleSettings, title: string, on: string, off: string) => (
    <SettingsRow
      title={title}
      description={s ? (s[key] ? on : off) : undefined}
      control={
        <Switch
          aria-label={title}
          checked={!!s?.[key]}
          disabled={!s || !!locked}
          {...(locked ? { disabledReason: locked } : {})}
          loading={save.isPending && save.variables?.[key] !== s?.[key]}
          onCheckedChange={(v) => s && save.mutate({ ...s, [key]: v })}
        />
      }
    />
  );
  return (
    <Section
      spacing="settings"
      title={t("project.lifecycle.title", "Model retirement")}
      description={t("project.lifecycle.hint", "What happens when a provider shuts down a model this project uses.")}
    >
      <SettingsGroup>
        {row(
          "move_on_shutdown",
          t("project.lifecycle.move", "Move aliases on shutdown"),
          t("project.lifecycle.moveOn", "On the shutdown day, aliases move to the replacement."),
          t("project.lifecycle.moveOff", "Aliases keep the model; move them yourself."),
        )}
        {row(
          "serve_replacement",
          t("project.lifecycle.serve", "Answer retired models with the replacement"),
          t("project.lifecycle.serveOn", "Apps that name a retired model get its replacement."),
          t("project.lifecycle.serveOff", "Requests to a retired model fail with 410."),
        )}
      </SettingsGroup>
    </Section>
  );
}
