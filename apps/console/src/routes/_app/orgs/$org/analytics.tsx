import { createFileRoute } from "@tanstack/react-router";
import { mcpServersQuery } from "~/api/queries/mcp-servers";
import { orgRights, sessionQuery } from "~/auth/session";
import { OrgAnalyticsPage } from "~/features/org-home/org-analytics-page";
import { periodSearch } from "~/format/period-search";
import { orgAnalyticsSearch } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/analytics")({
  validateSearch: orgAnalyticsSearch,
  loaderDeps: ({ search }) => periodSearch(search),
  loader: async ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchAnalytics(context.queryClient, params, deps));
    // Whether the organization has MCP servers decides the tool calls tab. The page waits for this short list, since its
    // first render starts before a prefetch alone would land, and a tab added after it narrows every other one.
    const session = context.queryClient.getQueryData(sessionQuery.queryKey);
    if (session && orgRights(session, params.org).has("mcp_servers.view")) await context.queryClient.prefetchQuery(mcpServersQuery(params.org));
  },
  component: Analytics,
});

function Analytics() {
  return <OrgAnalyticsPage org={Route.useParams().org} search={Route.useSearch({ select: orgAnalyticsSearch, structuralSharing: true })} />;
}
