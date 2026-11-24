import { createFileRoute } from "@tanstack/react-router";
import { orgProjectsQuery } from "~/api/queries/projects";
import { orgUsageQuery } from "~/api/queries/usage";
import { loadedOrgRights } from "~/auth/session";
import { resolvePeriod } from "~/format/presets";
import { OrgProjectsPage, type OrgProjectsSearch } from "~/features/org-home/org-projects-page";

const projectsSearch = (s: Record<string, unknown>): OrgProjectsSearch => ({
  ...(s.create === true || s.create === "true" ? { create: true } : {}),
  ...(s.sort === "requests" || s.sort === "cost" ? { sort: s.sort } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/")({
  validateSearch: projectsSearch,
  // The list and each project's requests and cost over the same 30 days the page shows.
  loader: ({ context: { queryClient }, params: { org } }) => {
    void queryClient.prefetchQuery(orgProjectsQuery(org));
    if (!loadedOrgRights(queryClient, org).has("usage.org.view")) return;
    const last30 = resolvePeriod({});
    void queryClient.prefetchQuery(orgUsageQuery(org, last30.from, last30.to, "project"));
  },
  component: Projects,
});

function Projects() {
  return <OrgProjectsPage org={Route.useParams().org} search={Route.useSearch({ select: projectsSearch, structuralSharing: true })} />;
}
