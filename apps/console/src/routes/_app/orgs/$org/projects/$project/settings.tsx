import { createFileRoute } from "@tanstack/react-router";
import { ProjectSettingsPage } from "~/features/project/settings/project-settings-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/settings")({
  component: ProjectSettings,
});

function ProjectSettings() {
  const { org, project } = Route.useParams();
  return <ProjectSettingsPage org={org} project={project} />;
}
