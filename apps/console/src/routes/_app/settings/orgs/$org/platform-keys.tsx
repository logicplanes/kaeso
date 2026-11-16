import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { PlatformKeysPage } from "~/features/org/platform-keys-page";

type PlatformKeysSearch = { key?: string };

const platformKeysSearch = (s: Record<string, unknown>): PlatformKeysSearch => (typeof s.key === "string" && s.key ? { key: s.key } : {});

export const Route = createFileRoute("/_app/settings/orgs/$org/platform-keys")({ validateSearch: platformKeysSearch, component: PlatformKeys });

function PlatformKeys() {
  const { org } = Route.useParams();
  const { key } = Route.useSearch();
  const navigate = useNavigate();
  return (
    <PlatformKeysPage
      org={org}
      open={key}
      onOpen={(next) => void navigate({ to: "/settings/orgs/$org/platform-keys", params: { org }, search: next ? { key: next } : {}, replace: true })}
    />
  );
}
