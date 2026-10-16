import { createFileRoute } from "@tanstack/react-router";
import { limitsQuery } from "~/api/queries/limits";
import { LimitsPage, type LimitsSearch } from "~/features/project/limits/limits-page";

const limitsSearch = (s: Record<string, unknown>): LimitsSearch => ({
  ...(s.level === "project" || s.level === "member" || s.level === "key" ? { level: s.level } : {}),
  ...(s.add === true || s.add === "true" ? { add: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/limits")({
  validateSearch: limitsSearch,
  loader: ({ context, params }) => context.queryClient.prefetchQuery(limitsQuery(params.org, params.project)),
  component: Limits,
});

function Limits() {
  const { org, project } = Route.useParams();
  return <LimitsPage org={org} project={project} search={Route.useSearch({ select: limitsSearch, structuralSharing: true })} />;
}
