import { createFileRoute } from "@tanstack/react-router";
import { OrgOverviewPage } from "~/features/org-home/org-overview-page";

export const Route = createFileRoute("/_app/orgs/$org/")({
  loader: () =>
    void Promise.all([import("~/features/org-home/org-glance"), import("~/features/org-home/home-news"), import("@logicplanes/kaeso-ui/charts/sparkline")]),
  component: Overview,
});

function Overview() {
  return <OrgOverviewPage org={Route.useParams().org} />;
}
