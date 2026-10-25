import { useQuery } from "@tanstack/react-query";
import { Segmented, SettingsGroup, SettingsRow, SettingsSection, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { agentCatalogueQuery, instanceAgentPolicyQuery } from "~/api/queries/agents";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

export function AgentsSettings() {
  const policy = useQuery(instanceAgentPolicyQuery);
  const cat = useQuery(agentCatalogueQuery);
  const save = useApiMutation({
    mutationFn: async (next: { allowed: boolean; max: string }) => data(await api.PUT("/platform/v1/instance/settings/agents", { body: next })),
    invalidates: [["instance", "agent-policy"]],
    onSuccess: () => toast.success(t("instance.agents.saved", "Agent policy saved")),
  });
  const p = policy.data;
  if (!p) return null;
  return (
    <SettingsSection
      title={t("instance.agents.title", "Agents")}
      description={t("instance.agents.description", "Agents sign in through the MCP server at /mcp and act for their user.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.agents.allow", "Allow agents")}
          description={t("instance.agents.allowHint", "Off signs every agent out at its next call.")}
          control={
            <Switch
              aria-label={t("instance.agents.allow", "Allow agents")}
              checked={p.allowed}
              loading={save.isPending}
              onCheckedChange={(allowed) => save.mutate({ allowed, max: p.max })}
            />
          }
        />
        {p.allowed && cat.data && (
          <SettingsRow
            title={t("instance.agents.max", "Most on the instance")}
            description={cat.data.instance_presets.find((x) => x.id === p.max)?.description}
            control={
              <Segmented
                aria-label={t("instance.agents.max", "Most on the instance")}
                size="sm"
                disabled={save.isPending}
                options={cat.data.instance_presets.map((x) => ({ value: x.id, label: x.name }))}
                value={p.max}
                onValueChange={(max) => save.mutate({ allowed: p.allowed, max })}
              />
            }
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}
