import { createFileRoute } from "@tanstack/react-router";
import { InstanceOrgPage } from "~/features/instance/org-page";

// Not `$org`: the shell reads `org` as the person's current organization, and an instance admin looks at any organization here.
export const Route = createFileRoute("/_app/settings/instance/orgs/$instanceOrg")({ component: Org });

function Org() {
  return <InstanceOrgPage org={Route.useParams().instanceOrg} />;
}
