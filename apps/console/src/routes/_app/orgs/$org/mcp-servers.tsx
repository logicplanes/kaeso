import { createFileRoute } from "@tanstack/react-router";
import { mcpServersQuery } from "~/api/queries/mcp-servers";
import { orgProjectsQuery } from "~/api/queries/projects";
import { loadedOrgRights } from "~/auth/session";
import { OrgMcpServersPage, type McpServersSearch, type SignInResult } from "~/features/org-home/mcp-servers-page";

const RESULTS: SignInResult[] = ["connected", "declined", "expired", "failed"];

const mcpServersSearch = (s: Record<string, unknown>): McpServersSearch => ({
  ...(typeof s.server === "string" && s.server ? { server: s.server } : {}),
  ...(RESULTS.includes(s.result as SignInResult) ? { result: s.result as SignInResult } : {}),
  ...(s.add === true || s.add === "true" ? { add: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/mcp-servers")({
  validateSearch: mcpServersSearch,
  loader: ({ context: { queryClient }, params: { org } }) => {
    if (!loadedOrgRights(queryClient, org).has("mcp_servers.view")) return;
    void queryClient.prefetchQuery(mcpServersQuery(org));
    void queryClient.prefetchQuery(orgProjectsQuery(org));
  },
  component: McpServers,
});

function McpServers() {
  return <OrgMcpServersPage org={Route.useParams().org} search={Route.useSearch({ select: mcpServersSearch, structuralSharing: true })} />;
}
