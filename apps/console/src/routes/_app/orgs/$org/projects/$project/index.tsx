import { createFileRoute } from "@tanstack/react-router";
import { limitsQuery } from "~/api/queries/limits";
import { projectRightsQuery, sessionQuery } from "~/auth/session";
import { preloadProjectOverview } from "~/features/project/overview-parts";
import { OverviewPage } from "~/features/project/overview-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/")({
  loader: ({ context: { queryClient }, params: { org, project } }) => {
    // Started here rather than when the page mounts, the split parts' code and the spend limit are in sooner, and a part
    // whose code is in by then draws with the page instead of when React reveals it. The project's gate loaded the rights.
    preloadProjectOverview();
    const session = queryClient.getQueryData(sessionQuery.queryKey);
    const access = session && queryClient.getQueryData(projectRightsQuery(session, org, project).queryKey);
    if (access?.rights.has("limits.view")) void queryClient.prefetchQuery(limitsQuery(org, project, { level: "project" }));
    void import("@logicplanes/kaeso-ui/charts/sparkline");
  },
  component: Overview,
});

function Overview() {
  const { org, project } = Route.useParams();
  return <OverviewPage org={org} project={project} />;
}
