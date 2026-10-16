import { createFileRoute } from "@tanstack/react-router";
import { OrgGeneralPage } from "~/features/org/general-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/general")({ component: General });

function General() {
  return <OrgGeneralPage org={Route.useParams().org} />;
}
