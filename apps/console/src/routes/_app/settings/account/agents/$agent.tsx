import { createFileRoute } from "@tanstack/react-router";
import { AgentPage, type AgentSearch } from "~/features/account/agent-page";

const agentSearch = (s: Record<string, unknown>): AgentSearch => ({
  ...(typeof s.add === "string" && s.add ? { add: s.add } : {}),
  ...(typeof s.org === "string" && s.org ? { org: s.org } : {}),
  ...(typeof s.project === "string" && s.project ? { project: s.project } : {}),
});

export const Route = createFileRoute("/_app/settings/account/agents/$agent")({ validateSearch: agentSearch, component: Agent });

function Agent() {
  return <AgentPage agent={Route.useParams().agent} search={Route.useSearch({ select: agentSearch, structuralSharing: true })} />;
}
