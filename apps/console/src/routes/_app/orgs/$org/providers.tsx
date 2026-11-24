import { createFileRoute } from "@tanstack/react-router";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { customProvidersQuery } from "~/api/queries/custom-providers";
import { orgMembersQuery, platformKeysQuery } from "~/api/queries/orgs";
import { loadedOrgRights } from "~/auth/session";
import { OrgProvidersPage, type ProvidersSearch } from "~/features/org-home/providers-page";

const providersSearch = (s: Record<string, unknown>): ProvidersSearch => (s.tab === "custom" ? { tab: "custom" } : {});

export const Route = createFileRoute("/_app/orgs/$org/providers")({
  validateSearch: providersSearch,
  // The keys, the custom providers, and the members and platform keys that name who added each key.
  loader: ({ context: { queryClient }, params: { org } }) => {
    const rights = loadedOrgRights(queryClient, org);
    if (rights.has("provider_credentials.view")) void queryClient.prefetchQuery(orgCredentialsQuery(org));
    if (rights.has("providers.custom.view")) void queryClient.prefetchQuery(customProvidersQuery(org));
    if (rights.has("members.view")) void queryClient.prefetchQuery(orgMembersQuery(org));
    if (rights.has("platform_keys.view")) void queryClient.prefetchQuery(platformKeysQuery(org, true));
  },
  component: Providers,
});

function Providers() {
  return <OrgProvidersPage org={Route.useParams().org} search={Route.useSearch({ select: providersSearch, structuralSharing: true })} />;
}
