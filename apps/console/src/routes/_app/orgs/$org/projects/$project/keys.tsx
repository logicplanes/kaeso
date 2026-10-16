import { createFileRoute } from "@tanstack/react-router";
import { allKeysQuery, keyFilter } from "~/api/queries/keys";
import { KeysPage, type KeysSearch } from "~/features/project/keys/keys-page";

const keysSearch = (s: Record<string, unknown>): KeysSearch => ({
  ...(typeof s.key === "string" ? { key: s.key } : {}),
  ...(typeof s.owner === "string" ? { owner: s.owner } : {}),
  ...(s.archived === true || s.archived === "true" ? { archived: true } : {}),
  ...(s.create === true || s.create === "true" ? { create: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/keys")({
  validateSearch: keysSearch,
  loaderDeps: ({ search }) => {
    const { owner, archived } = keysSearch(search);
    return { owner, archived };
  },
  loader: ({ context, params, deps }) => context.queryClient.prefetchQuery(allKeysQuery(params.org, params.project, keyFilter(deps))),
  component: Keys,
});

function Keys() {
  const { org, project } = Route.useParams();
  return <KeysPage org={org} project={project} search={Route.useSearch({ select: keysSearch, structuralSharing: true })} />;
}
