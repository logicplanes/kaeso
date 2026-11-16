import { createFileRoute } from "@tanstack/react-router";
import { WebhooksPage } from "~/features/org/webhooks-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/webhooks")({ component: Webhooks });

function Webhooks() {
  return <WebhooksPage org={Route.useParams().org} />;
}
