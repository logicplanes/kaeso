import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/orgs/$org/provider-keys")({
  beforeLoad: ({ params }) => {
    throw redirect({ to: "/orgs/$org/providers", params: { org: params.org } });
  },
});
