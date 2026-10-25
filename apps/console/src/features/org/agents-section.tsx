import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AgentAttribution, AlertDialog, Button, Segmented, SettingsGroup, SettingsRow, SettingsSection, Switch } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { modelLabel } from "~/format/model";
import { api, data } from "~/api/client";
import type { OrgAgentView } from "~/api/generated/types";
import { agentCatalogueQuery, orgAgentPolicyQuery, orgAgentsQuery } from "~/api/queries/agents";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

export function OrgAgents({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const canManage = rights.has("members.rights.update");
  const policy = useQuery(orgAgentPolicyQuery(org));
  const cat = useQuery(agentCatalogueQuery);
  const agents = useQuery({ ...orgAgentsQuery(org), enabled: rights.has("members.view") });
  const [closing, setClosing] = useState<OrgAgentView[]>([]);
  const save = useApiMutation({
    mutationFn: async (next: { allowed: boolean; max: string }) =>
      data(await api.PUT("/platform/v1/orgs/{org}/security/agents", { params: { path: { org } }, body: next })),
    invalidates: [
      ["orgs", org, "agent-policy"],
      ["orgs", org, "agents"],
    ],
    onSuccess: () => toast.success(t("org.agents.saved", "Agent policy saved")),
  });
  const close = useApiMutation({
    mutationFn: async (chosen: OrgAgentView[]) => {
      for (const a of chosen) await api.DELETE("/platform/v1/orgs/{org}/agents/{agent}", { params: { path: { org, agent: a.id } } });
    },
    invalidates: [["orgs", org, "agents"]],
    onSuccess: () => {
      setClosing([]);
      toast(t("org.agents.closed", "Agent can no longer act here"));
    },
  });
  const p = policy.data;
  const locked = canManage ? {} : { disabledReason: needs("members.rights.update") };
  return (
    <SettingsSection
      title={t("org.agents.title", "Agents")}
      description={t("org.agents.description", "Members' agents act here only with rights the member holds and this allows.")}
    >
      {p && (
        <SettingsGroup>
          <SettingsRow
            title={t("org.agents.allow", "Allow members' agents")}
            description={t("org.agents.allowHint", "Off stops every agent here at its next call.")}
            control={
              <Switch
                aria-label={t("org.agents.allow", "Allow members' agents")}
                checked={p.allowed}
                disabled={!canManage}
                {...locked}
                loading={save.isPending}
                onCheckedChange={(allowed) => save.mutate({ allowed, max: p.max })}
              />
            }
          />
          {p.allowed && cat.data && (
            <SettingsRow
              title={t("org.agents.max", "Most an agent may do")}
              description={cat.data.presets.find((x) => x.id === p.max)?.description}
              control={
                <Segmented
                  aria-label={t("org.agents.max", "Most an agent may do")}
                  size="sm"
                  disabled={!canManage || save.isPending}
                  options={cat.data.presets.map((x) => ({ value: x.id, label: x.name }))}
                  value={p.max}
                  onValueChange={(max) => save.mutate({ allowed: p.allowed, max })}
                />
              }
            />
          )}
        </SettingsGroup>
      )}
      {agents.data && agents.data.length > 0 && (
        <SettingsGroup>
          {agents.data.map((a) => (
            <SettingsRow
              key={a.id}
              title={
                <span className="flex items-center gap-8">
                  <AgentAttribution name={a.user.name} src={a.user.avatar_url ?? undefined} agent={{ name: a.name, model: a.model }} size={24} />
                  {t("org.agents.agentOf", "{agent} of {person}", { agent: a.name, person: a.user.name })}
                </span>
              }
              description={t("org.agents.agentDetail", "{model} · used {used}", {
                model: modelLabel(a.model),
                used: a.last_used_at ? formatRelative(a.last_used_at) : t("org.agents.neverUsed", "never"),
              })}
              control={
                <Button size="sm" variant="ghost" disabled={!canManage} {...locked} onClick={() => setClosing([a])}>
                  {t("org.agents.close", "Close access")}
                </Button>
              }
            />
          ))}
        </SettingsGroup>
      )}
      <AlertDialog
        open={closing.length > 0}
        onOpenChange={(o) => !o && setClosing([])}
        tone="danger"
        title={t("org.agents.closeConfirm", "Close this organization to {agent}?", { agent: closing.map((a) => a.name).join("") })}
        description={t("org.agents.closeConfirmHint", "It stops acting here at once; its member can open the organization to it again.")}
        confirmLabel={t("org.agents.close", "Close access")}
        loading={close.isPending}
        onConfirm={() => close.mutateAsync(closing)}
      />
    </SettingsSection>
  );
}
