import { createFileRoute } from "@tanstack/react-router";
import { aliasQuery, aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { RouteEditorPage } from "~/features/project/models/route-editor/route-editor-page";

const NEW = "new";

type AliasSearch = { from?: string };

// `from` names the alias a new one starts as a copy of.
const aliasSearch = (s: Record<string, unknown>): AliasSearch => (typeof s.from === "string" && s.from ? { from: s.from } : {});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/models_/aliases/$alias")({
  validateSearch: aliasSearch,
  loaderDeps: ({ search }) => ({ from: aliasSearch(search).from }),
  loader: ({ context: { queryClient }, params: { org, project, alias }, deps: { from } }) =>
    Promise.all([
      queryClient.prefetchQuery(catalogueQuery(org, project)),
      alias !== NEW && queryClient.prefetchQuery(aliasQuery(org, project, alias)),
      alias === NEW && from && queryClient.prefetchQuery(aliasQuery(org, project, from)),
      alias === NEW && from && queryClient.prefetchQuery(aliasesQuery(org, project)),
    ]),
  component: AliasRoute,
});

function AliasRoute() {
  const { org, project, alias } = Route.useParams();
  const { from } = Route.useSearch();
  const isNew = alias === NEW;
  return (
    <RouteEditorPage key={isNew ? `new:${from ?? ""}` : alias} org={org} project={project} alias={isNew ? undefined : alias} from={isNew ? from : undefined} />
  );
}
