import { createFileRoute } from "@tanstack/react-router";
import { aliasQuery, catalogueQuery } from "~/api/queries/models";
import { RouteEditorPage } from "~/features/project/models/route-editor/route-editor-page";

const NEW = "new";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/models_/aliases/$alias")({
  loader: ({ context: { queryClient }, params: { org, project, alias } }) =>
    Promise.all([queryClient.prefetchQuery(catalogueQuery(org, project)), alias !== NEW && queryClient.prefetchQuery(aliasQuery(org, project, alias))]),
  component: AliasRoute,
});

function AliasRoute() {
  const { org, project, alias } = Route.useParams();
  return <RouteEditorPage key={alias} org={org} project={project} alias={alias === NEW ? undefined : alias} />;
}
