import { createFileRoute } from "@tanstack/react-router";
import { validateAuditSearch } from "~/api/queries/audit-search";
import { ProjectAuditLogPage } from "~/features/project/audit-log/audit-log-page";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/audit-log")({
  validateSearch: validateAuditSearch,
  loaderDeps: ({ search }) => {
    const { event: _event, ...filters } = validateAuditSearch(search);
    return filters;
  },
  loader: ({ context, params, deps }) => {
    void import("~/api/queries/route-prefetch").then((m) => m.prefetchProjectAuditLog(context.queryClient, params.org, params.project, deps));
  },
  component: AuditLog,
});

function AuditLog() {
  const { org, project } = Route.useParams();
  return <ProjectAuditLogPage org={org} project={project} search={Route.useSearch({ select: validateAuditSearch, structuralSharing: true })} />;
}
