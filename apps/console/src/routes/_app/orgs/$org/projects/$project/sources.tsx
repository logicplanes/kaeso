import { createFileRoute } from "@tanstack/react-router";
import { sourcesQuery } from "~/api/queries/sources";
import { SourcesPage, type SourcesSearch } from "~/features/project/sources/sources-page";

const sourcesSearch = (s: Record<string, unknown>): SourcesSearch => ({
  ...(typeof s.source === "string" ? { source: s.source } : {}),
  ...(s.scope === "archived" || s.scope === "all" ? { scope: s.scope } : {}),
  ...(s.add === true || s.add === "true" ? { add: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/sources")({
  validateSearch: sourcesSearch,
  loader: ({ context, params }) => context.queryClient.prefetchQuery(sourcesQuery(params.org, params.project)),
  component: Sources,
});

function Sources() {
  const { org, project } = Route.useParams();
  return <SourcesPage org={org} project={project} search={Route.useSearch({ select: sourcesSearch, structuralSharing: true })} />;
}
