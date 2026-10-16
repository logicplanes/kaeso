import { createFileRoute } from "@tanstack/react-router";
import { OrgSecurityPage } from "~/features/org/security-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/security")({ component: Security });

function Security() {
  return <OrgSecurityPage org={Route.useParams().org} />;
}
