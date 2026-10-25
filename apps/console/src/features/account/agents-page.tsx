import { useQuery } from "@tanstack/react-query";
import { Badge, Code, CopyButton, Empty, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { modelLabel } from "~/format/model";
import type { AgentView } from "~/api/generated/types";
import { agentCatalogueQuery, agentsQuery, grantableQuery } from "~/api/queries/agents";
import { ConnectClients, mcpUrl } from "~/features/account/connect-client";
import { LoadFailed } from "~/features/org/load-failed";
import { t } from "~/i18n";
import { fromGrant, summary } from "~/kaeso/agent-grant";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";

export function agentDates(a: Pick<AgentView, "created_at" | "last_used_at">): string {
  return t("account.agentAddedUsed", "Added {created} · used {used}", {
    created: formatDate(a.created_at),
    used: a.last_used_at ? formatRelative(a.last_used_at) : t("account.agentNeverUsed", "never"),
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
      description={t("account.agentsHint", "Add the MCP server to an agent; it acts with the rights you give it.")}
    >
      <SettingsSection>
        <SettingsGroup>
          <SettingsRow
            title={t("account.mcpServer", "MCP server")}
            description={
              <span className="flex min-w-0 items-center gap-8">
                <Code>{mcpUrl()}</Code>
                <CopyButton value={mcpUrl()} label={t("account.copyMcpUrl", "Copy the MCP server address")} size="xs" />
              </span>
            }
            control={<ConnectClients />}
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
                    {a.name}
                    <Badge>{modelLabel(a.model)}</Badge>
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
