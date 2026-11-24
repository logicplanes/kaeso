import { createFileRoute } from "@tanstack/react-router";
import { projectGuardrailsQuery } from "~/api/queries/guardrails";
import { orgQuery } from "~/api/queries/orgs";
import { modelLifecycleQuery, projectLogSettingsQuery, projectResponseCacheQuery, projectStoredResponsesQuery } from "~/api/queries/project-settings";
import { loadedProjectRights } from "~/auth/rights";
import { sessionQuery } from "~/auth/session";
import { ProjectSettingsPage } from "~/features/project/settings/project-settings-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/settings")({
  // Each section of the page reads its own setting; these are the sections the page shows with the person's rights.
  loader: ({ context: { queryClient }, params: { org, project } }) => {
    const rights = loadedProjectRights(queryClient, org, project);
    if (queryClient.getQueryData(sessionQuery.queryKey)?.request_logs) void queryClient.prefetchQuery(projectLogSettingsQuery(org, project));
    if (rights.has("security.key_policy.project.manage")) void queryClient.prefetchQuery(orgQuery(org));
    void queryClient.prefetchQuery(projectResponseCacheQuery(org, project));
    void queryClient.prefetchQuery(projectGuardrailsQuery(org, project));
    void queryClient.prefetchQuery(projectStoredResponsesQuery(org, project));
    if (rights.has("models.view")) void queryClient.prefetchQuery(modelLifecycleQuery(org, project));
  },
  component: ProjectSettings,
});

function ProjectSettings() {
  const { org, project } = Route.useParams();
  return <ProjectSettingsPage org={org} project={project} />;
}
