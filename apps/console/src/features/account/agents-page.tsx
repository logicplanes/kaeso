import { useQuery } from "@tanstack/react-query";
import { AgentAvatar, Badge, Empty, InlineCopyText, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { harnessLabel } from "~/format/harness";
import type { AgentView } from "~/api/generated/types";
import { agentCatalogueQuery, agentsQuery, grantableQuery } from "~/api/queries/agents";
import { LoadFailed } from "~/features/org/load-failed";
import { t } from "~/i18n";
import { fromGrant, summary } from "~/kaeso/agent-grant";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";

export const mcpUrl = (): string => `${window.location.origin}/mcp`;

export function agentDates(a: Pick<AgentView, "created_at" | "last_used_at" | "expires_at">): string {
  return t("account.agentDates", "Added {created} · used {used} · {expires}", {
    created: formatDate(a.created_at),
    used: a.last_used_at ? formatRelative(a.last_used_at) : t("account.agentNeverUsed", "never"),
    expires: a.expires_at ? t("account.agentExpires", "ends {when}", { when: formatDate(a.expires_at) }) : t("account.agentNoExpiry", "no end date"),
  });
}

export function AgentsPage() {
  const agents = useQuery(agentsQuery);
  const cat = useQuery(agentCatalogueQuery);
  const grantable = useQuery(grantableQuery);
  const list = agents.data ?? [];
  return (
    <SettingsPage
      title={t("account.agents", "Agents")}
      context={t("account.title", "Account")}
      description={t("account.agentsHint", "Agents act for you through Kaeso's MCP server with the rights you gave them.")}
    >
      <SettingsSection>
        <SettingsGroup>
          <SettingsRow
            title={t("account.mcpServer", "MCP server")}
            description={t("account.mcpServerHint", "Add this address to your agent; it signs in here.")}
            control={<InlineCopyText value={mcpUrl()} label={t("account.copyMcpUrl", "Copy the MCP server address")} />}
          />
        </SettingsGroup>
      </SettingsSection>
      {agents.isError ? (
        <LoadFailed title={t("account.agentsFailed", "Your agents couldn't load")} />
      ) : agents.isPending ? (
        <SettingsLoading rows={2} />
      ) : list.length === 0 ? (
        <Empty
          title={t("account.noAgents", "No agents yet")}
          description={t("account.noAgentsHint", "Add the MCP server to Claude Code, Codex or another agent and sign in from there.")}
        />
      ) : (
        <SettingsSection>
          <SettingsGroup>
            {list.map((a) => (
              <SettingsRow
                key={a.id}
                href={`/settings/account/agents/${a.id}`}
                title={
                  <span className="flex flex-wrap items-center gap-8">
                    <AgentAvatar harness={a.harness} name={a.name} size={24} decorative />
                    {a.name}
                    <Badge>{harnessLabel(a.harness)}</Badge>
                  </span>
                }
                description={
                  <span className="flex flex-col gap-2">
                    {cat.data && grantable.data && <span>{summary(cat.data, grantable.data, fromGrant(a.grant, grantable.data))}</span>}
                    <span>{agentDates(a)}</span>
                  </span>
                }
              />
            ))}
          </SettingsGroup>
        </SettingsSection>
      )}
    </SettingsPage>
  );
}
