import { createFileRoute } from "@tanstack/react-router";
import { projectServersQuery } from "~/api/queries/tools";
import { projectRightsQuery, sessionQuery } from "~/auth/session";
import { AnalyticsPage } from "~/features/project/analytics-page";
import { periodSearch } from "~/format/period-search";
import { analyticsSearch } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/analytics")({
  validateSearch: analyticsSearch,
  loaderDeps: ({ search }) => periodSearch(search),
  loader: async ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchAnalytics(context.queryClient, params, deps));
    // Whether the project has MCP servers decides the tool calls tab. The page waits for this short list, since its
    // first render starts before a prefetch alone would land, and a tab added after it narrows every other one.
    const session = context.queryClient.getQueryData(sessionQuery.queryKey);
    const access = session && context.queryClient.getQueryData(projectRightsQuery(session, params.org, params.project).queryKey);
    if (access?.rights.has("tools.view")) await context.queryClient.prefetchQuery(projectServersQuery(params.org, params.project));
  },
  component: Analytics,
});

function Analytics() {
  const { org, project } = Route.useParams();
  return <AnalyticsPage org={org} project={project} search={Route.useSearch({ select: analyticsSearch, structuralSharing: true })} />;
}
