import { createFileRoute } from "@tanstack/react-router";
import { GuardrailsPage } from "~/features/org/guardrails-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/guardrails")({ component: Guardrails });

function Guardrails() {
  return <GuardrailsPage org={Route.useParams().org} />;
}
