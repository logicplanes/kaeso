import { createFileRoute } from "@tanstack/react-router";
import { SharedLinksPage } from "~/features/org/shared-links-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/shared-links")({ component: SharedLinks });

function SharedLinks() {
  return <SharedLinksPage org={Route.useParams().org} />;
}
