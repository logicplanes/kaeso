import { createFileRoute } from "@tanstack/react-router";
import { validateAuditSearch } from "~/api/queries/audit-search";
import { InstanceAuditLogPage } from "~/features/instance/audit-log-page";

export const Route = createFileRoute("/_app/settings/instance/audit-log")({ validateSearch: validateAuditSearch, component: AuditLog });

function AuditLog() {
  return <InstanceAuditLogPage search={Route.useSearch({ select: validateAuditSearch, structuralSharing: true })} />;
}
