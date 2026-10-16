import { createFileRoute } from "@tanstack/react-router";
import { validateAuditSearch } from "~/api/queries/audit-search";
import { OrgAuditLogPage } from "~/features/org/audit-log-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/audit-log")({ validateSearch: validateAuditSearch, component: AuditLog });

function AuditLog() {
  return <OrgAuditLogPage org={Route.useParams().org} search={Route.useSearch({ select: validateAuditSearch, structuralSharing: true })} />;
}
