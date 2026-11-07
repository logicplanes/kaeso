import { createFileRoute, notFound } from "@tanstack/react-router";
import { OrgAnalyticsPage } from "~/features/org-home/org-analytics-page";
import { ORG_VIEWS, orgAnalyticsSearch, type OrgView } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/analytics_/$view")({
  validateSearch: orgAnalyticsSearch,
  beforeLoad: ({ params }) => {
    if (!ORG_VIEWS.includes(params.view as OrgView)) throw notFound();
  },
  component: AnalyticsView,
});

function AnalyticsView() {
  const { org, view } = Route.useParams();
  return <OrgAnalyticsPage org={org} view={view as OrgView} search={Route.useSearch({ select: orgAnalyticsSearch, structuralSharing: true })} />;
}
