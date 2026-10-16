import { createFileRoute } from "@tanstack/react-router";
import { InstanceOrgPage } from "~/features/instance/org-page";

export const Route = createFileRoute("/_app/settings/instance/orgs/$org")({ component: Org });

function Org() {
  return <InstanceOrgPage org={Route.useParams().org} />;
}
