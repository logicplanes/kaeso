import { createFileRoute } from "@tanstack/react-router";
import { PlaygroundPage } from "~/features/project/playground/playground-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/playground")({
  component: Playground,
});

function Playground() {
  const { org, project } = Route.useParams();
  return <PlaygroundPage org={org} project={project} search={Route.useSearch()} />;
}
