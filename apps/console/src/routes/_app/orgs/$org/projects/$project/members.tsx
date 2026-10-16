import { createFileRoute } from "@tanstack/react-router";
import { projectMembersQuery } from "~/api/queries/members";
import { MembersPage, type MembersSearch } from "~/features/project/members/members-page";

const membersSearch = (s: Record<string, unknown>): MembersSearch => ({
  ...(typeof s.member === "string" ? { member: s.member } : {}),
  ...(s.add === true || s.add === "true" ? { add: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/members")({
  validateSearch: membersSearch,
  loader: ({ context, params }) => context.queryClient.prefetchQuery(projectMembersQuery(params.org, params.project)),
  component: Members,
});

function Members() {
  const { org, project } = Route.useParams();
  return <MembersPage org={org} project={project} search={Route.useSearch({ select: membersSearch, structuralSharing: true })} />;
}
