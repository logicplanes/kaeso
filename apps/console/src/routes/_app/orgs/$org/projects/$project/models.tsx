import { createFileRoute } from "@tanstack/react-router";
import type { Capability } from "@logicplanes/kaeso-ui";
import { catalogueQuery } from "~/api/queries/models";
import { ModelsPage, type ModelsSearch, type ModelsSort } from "~/features/project/models/models-page";

const CAPS = new Set<string>(["tools", "reasoning", "structuredOutput", "vision", "pdf"]);
const SORTS = new Set<string>(["price", "release"]);

const modelsSearch = (s: Record<string, unknown>): ModelsSearch => ({
  ...(s.tab === "aliases" ? { tab: "aliases" as const } : {}),
  ...(typeof s.model === "string" ? { model: s.model } : {}),
  ...(typeof s.provider === "string" ? { provider: s.provider } : {}),
  ...(typeof s.cap === "string" && CAPS.has(s.cap) ? { cap: s.cap as Capability } : {}),
  ...(s.overridden === true || s.overridden === "true" ? { overridden: true } : {}),
  ...(typeof s.sort === "string" && SORTS.has(s.sort) ? { sort: s.sort as ModelsSort } : {}),
  ...(s.add === true || s.add === "true" ? { add: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/models")({
  validateSearch: modelsSearch,
  loader: ({ context, params }) => context.queryClient.prefetchQuery(catalogueQuery(params.org, params.project)),
  component: Models,
});

function Models() {
  const { org, project } = Route.useParams();
  return <ModelsPage org={org} project={project} search={Route.useSearch({ select: modelsSearch, structuralSharing: true })} />;
}
