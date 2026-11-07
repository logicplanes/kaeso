import { createFileRoute, notFound } from "@tanstack/react-router";
import { AnalyticsPage } from "~/features/project/analytics-page";
import { analyticsSearch, PROJECT_VIEWS, type ProjectView } from "~/kaeso/overview/views";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/analytics_/$view")({
  validateSearch: analyticsSearch,
  beforeLoad: ({ params }) => {
    if (!PROJECT_VIEWS.includes(params.view as ProjectView)) throw notFound();
  },
  component: AnalyticsView,
});

function AnalyticsView() {
  const { org, project, view } = Route.useParams();
  return (
    <AnalyticsPage org={org} project={project} view={view as ProjectView} search={Route.useSearch({ select: analyticsSearch, structuralSharing: true })} />
  );
}
