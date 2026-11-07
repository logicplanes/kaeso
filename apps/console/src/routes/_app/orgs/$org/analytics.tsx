import { createFileRoute } from "@tanstack/react-router";
import { OrgAnalyticsPage } from "~/features/org-home/org-analytics-page";
import { orgAnalyticsSearch } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/analytics")({ validateSearch: orgAnalyticsSearch, component: Analytics });

function Analytics() {
  return <OrgAnalyticsPage org={Route.useParams().org} search={Route.useSearch({ select: orgAnalyticsSearch, structuralSharing: true })} />;
}
