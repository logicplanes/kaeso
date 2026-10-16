import { createFileRoute } from "@tanstack/react-router";
import { OrgProvidersPage } from "~/features/org-home/providers-page";

export const Route = createFileRoute("/_app/orgs/$org/providers")({ component: Providers });

function Providers() {
  return <OrgProvidersPage org={Route.useParams().org} />;
}
