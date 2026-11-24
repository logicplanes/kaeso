import { createFileRoute } from "@tanstack/react-router";
import { OrgAnalyticsPage } from "~/features/org-home/org-analytics-page";
import { periodSearch } from "~/format/period-search";
import { orgAnalyticsSearch } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/analytics")({
  validateSearch: orgAnalyticsSearch,
  loaderDeps: ({ search }) => periodSearch(search),
  loader: ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchAnalytics(context.queryClient, params, deps));
  },
  component: Analytics,
});

function Analytics() {
  return <OrgAnalyticsPage org={Route.useParams().org} search={Route.useSearch({ select: orgAnalyticsSearch, structuralSharing: true })} />;
}
