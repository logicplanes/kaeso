import { createFileRoute } from "@tanstack/react-router";
import { orgProjectsQuery, orgRetirementsQuery } from "~/api/queries/projects";
import { monthByProjectQuery } from "~/api/queries/usage";
import { whatsNewQuery } from "~/api/queries/whats-new";
import { orgRights, sessionQuery } from "~/auth/session";
import { preloadOrgOverview } from "~/features/org-home/org-overview-parts";
import { utcDay } from "~/format/period";
import { OrgOverviewPage } from "~/features/org-home/org-overview-page";

export const Route = createFileRoute("/_app/orgs/$org/")({
  loader: ({ context: { queryClient }, params: { org } }) => {
    // Started here rather than when the page mounts, the split parts' code and data are in sooner, and a part whose code
    // is in by then draws with the page instead of when React reveals it.
    preloadOrgOverview();
    void queryClient.prefetchQuery(orgRetirementsQuery(org));
    void queryClient.prefetchQuery(whatsNewQuery);
    void queryClient.prefetchQuery(orgProjectsQuery(org, true));
    const session = queryClient.getQueryData(sessionQuery.queryKey);
    if (session && orgRights(session, org).has("costs.org.view")) void queryClient.prefetchQuery(monthByProjectQuery(org, utcDay(new Date())));
    void import("@logicplanes/kaeso-ui/charts/sparkline");
  },
  component: Overview,
});

function Overview() {
  return <OrgOverviewPage org={Route.useParams().org} />;
}
