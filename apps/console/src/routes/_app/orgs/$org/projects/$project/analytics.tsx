import { createFileRoute } from "@tanstack/react-router";
import { AnalyticsPage } from "~/features/project/analytics-page";
import { periodSearch } from "~/format/period-search";
import { analyticsSearch } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/analytics")({
  validateSearch: analyticsSearch,
  loaderDeps: ({ search }) => periodSearch(search),
  loader: ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchProjectAnalytics(context.queryClient, params.org, params.project, deps));
  },
  component: Analytics,
});

function Analytics() {
  const { org, project } = Route.useParams();
  return <AnalyticsPage org={org} project={project} search={Route.useSearch({ select: analyticsSearch, structuralSharing: true })} />;
}
