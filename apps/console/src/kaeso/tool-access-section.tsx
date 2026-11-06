import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Checkbox, CheckboxGroup, Section, Switch, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ToolAccess } from "~/api/generated/types";
import { projectToolViews } from "~/api/queries/invalidation";
import { projectToolsQuery, toolAccessQuery } from "~/api/queries/tools";
import { useApiMutation } from "~/auth/mutation";
import { joinList } from "~/format/labels";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

export type ToolSubject = { principal: string; owner?: string | null | undefined };

export function ToolAccessSection({ org, project, subject, canEdit }: { org: string; project: string; subject: ToolSubject; canEdit: boolean }) {
  const access = useQuery(toolAccessQuery(org, project, subject.principal));
  const owner = useQuery({ ...toolAccessQuery(org, project, subject.owner ?? ""), enabled: canEdit && !!subject.owner });
  const tools = useQuery(projectToolsQuery(org, project));
  const [editing, setEditing] = useState<{ all: boolean; tools: string[] } | null>(null);
  const name = (id: string) => tools.data?.find((x) => x.id === id)?.exposed_name ?? id;
  const choices = subject.owner ? (owner.data?.effective ?? []) : (tools.data ?? []).filter((x) => x.offered).map((x) => x.id);
  const above = subject.owner
    ? t("kaeso.toolAccess.ownerTools", "Every tool its owner may use")
    : t("kaeso.toolAccess.projectTools", "Every tool the project offers");

  const save = useApiMutation({
    mutationFn: async (next: { all: boolean; tools: string[] }): Promise<ToolAccess> =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/projects/{project}/tool-access/{principal}", {
          params: { path: { org, project, principal: subject.principal } },
          body: next.all ? { all: true } : { tools: next.tools },
        }),
      ),
    invalidates: [projectToolViews(org)],
    onSuccess: () => {
      setEditing(null);
      toast.success(t("kaeso.toolAccess.saved", "Tools saved"));
    },
  });

  const a = access.data;
  return (
    <Section
      title={t("kaeso.toolAccess.title", "Tools")}
      spacing="none"
      actions={
        canEdit && editing === null && a ? (
          <Button size="sm" variant="ghost" aria-label={t("kaeso.toolAccess.edit", "Edit tools")} onClick={() => setEditing({ all: a.all, tools: a.tools })}>
            {t("common.edit", "Edit")}
          </Button>
        ) : undefined
      }
    >
      {editing !== null ? (
        <div className="flex flex-col gap-12">
          <Switch label={above} checked={editing.all} onCheckedChange={(all) => setEditing({ ...editing, all })} />
          {!editing.all && (
            <CheckboxGroup aria-label={t("kaeso.toolAccess.title", "Tools")} value={editing.tools} onValueChange={(v) => setEditing({ ...editing, tools: v })}>
              {[...new Set([...choices, ...editing.tools])].map((id) => (
                <Checkbox key={id} value={id} label={name(id)} />
              ))}
            </CheckboxGroup>
          )}
          <div className="flex gap-8">
            <Button size="sm" variant="primary" loading={save.isPending} onClick={() => save.mutate(editing)}>
              {t("common.save", "Save")}
            </Button>
            <Button size="sm" onClick={() => setEditing(null)}>
              {t("common.cancel", "Cancel")}
            </Button>
          </div>
        </div>
      ) : !a ? (
        <Text tone="secondary">{t("kaeso.toolAccess.loading", "Loading tools…")}</Text>
      ) : a.all ? (
        <Text tone="secondary">
          {a.effective.length === 0
            ? t("kaeso.toolAccess.none", "No tools to call yet.")
            : subject.owner
              ? t(
                  "kaeso.toolAccess.ownerCount",
                  "{count, plural, one {Every tool its owner may use: # tool.} other {Every tool its owner may use: # tools.}}",
                  {
                    count: a.effective.length,
                  },
                )
              : t(
                  "kaeso.toolAccess.projectCount",
                  "{count, plural, one {Every tool the project offers: # tool.} other {Every tool the project offers: # tools.}}",
                  {
                    count: a.effective.length,
                  },
                )}
        </Text>
      ) : (
        <Text>{joinList(a.tools.map(name)) || t("common.none", "None")}</Text>
      )}
    </Section>
  );
}
