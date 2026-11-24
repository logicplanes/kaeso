import { createFileRoute } from "@tanstack/react-router";
import { portalGrantsQuery, projectServersQuery, projectToolsQuery } from "~/api/queries/tools";
import { ToolsPage, type ToolsSearch } from "~/features/project/tools/tools-page";

const one = <T extends string>(v: unknown, allowed: readonly T[]): T | undefined => allowed.find((a) => a === v);
const text = (v: unknown) => (typeof v === "string" && v ? v : undefined);

const toolsSearch = (s: Record<string, unknown>): ToolsSearch => {
  const out: ToolsSearch = {};
  const tab = one(s.tab, ["servers", "connect"] as const);
  const state = one(s.state, ["on", "off", "changed"] as const);
  const hint = one(s.hint, ["read_only", "destructive", "open_world"] as const);
  const sort = one(s.sort, ["calls", "used", "on"] as const);
  const tool = text(s.tool);
  const server = text(s.server);
  if (tab) out.tab = tab;
  if (tool) out.tool = tool;
  if (server) out.server = server;
  if (state) out.state = state;
  if (hint) out.hint = hint;
  if (sort) out.sort = sort;
  return out;
};

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/tools")({
  validateSearch: toolsSearch,
  // Every tab's count is in the header, so the page asks for all three lists whichever tab is open.
  loader: ({ context: { queryClient }, params: { org, project } }) => {
    void queryClient.prefetchQuery(projectToolsQuery(org, project));
    void queryClient.prefetchQuery(projectServersQuery(org, project));
    void queryClient.prefetchQuery(portalGrantsQuery(org, project));
  },
  component: Tools,
});

function Tools() {
  const { org, project } = Route.useParams();
  return <ToolsPage org={org} project={project} search={Route.useSearch({ select: toolsSearch, structuralSharing: true })} />;
}
