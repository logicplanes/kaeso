import { createFileRoute } from "@tanstack/react-router";
import { OverviewPage, type OverviewSearch } from "~/features/project/overview-page";
import { periodSearch } from "~/format/period-search";

const BY = ["day", "model", "key", "member"] as const;

function overviewSearch(search: Record<string, unknown>): OverviewSearch {
  const by = BY.find((b) => b === search.by);
  return { ...periodSearch(search), ...(by ? { by } : {}) };
}

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/")({
  validateSearch: overviewSearch,
  loaderDeps: ({ search }) => periodSearch(search),
  loader: ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchProjectOverview(context.queryClient, params.org, params.project, deps));
  },
  component: Overview,
});

function Overview() {
  const { org, project } = Route.useParams();
  return <OverviewPage org={org} project={project} search={Route.useSearch({ select: overviewSearch, structuralSharing: true })} />;
}
