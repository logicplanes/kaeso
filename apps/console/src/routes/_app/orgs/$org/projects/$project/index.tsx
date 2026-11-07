import { createFileRoute } from "@tanstack/react-router";
import { OverviewPage } from "~/features/project/overview-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/")({
  loader: () =>
    void Promise.all([import("~/features/project/overview-glance"), import("~/kaeso/overview/limit"), import("@logicplanes/kaeso-ui/charts/sparkline")]),
  component: Overview,
});

function Overview() {
  const { org, project } = Route.useParams();
  return <OverviewPage org={org} project={project} />;
}
