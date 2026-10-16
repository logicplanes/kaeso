import { createFileRoute } from "@tanstack/react-router";
import { PlatformKeysPage } from "~/features/org/platform-keys-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/platform-keys")({ component: PlatformKeys });

function PlatformKeys() {
  return <PlatformKeysPage org={Route.useParams().org} />;
}
