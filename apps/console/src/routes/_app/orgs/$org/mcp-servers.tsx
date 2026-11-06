import { createFileRoute } from "@tanstack/react-router";
import { OrgMcpServersPage, type McpServersSearch, type SignInResult } from "~/features/org-home/mcp-servers-page";

const RESULTS: SignInResult[] = ["connected", "declined", "expired", "failed"];

const mcpServersSearch = (s: Record<string, unknown>): McpServersSearch => ({
  ...(typeof s.server === "string" && s.server ? { server: s.server } : {}),
  ...(RESULTS.includes(s.result as SignInResult) ? { result: s.result as SignInResult } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/mcp-servers")({ validateSearch: mcpServersSearch, component: McpServers });

function McpServers() {
  return <OrgMcpServersPage org={Route.useParams().org} search={Route.useSearch({ select: mcpServersSearch, structuralSharing: true })} />;
}
