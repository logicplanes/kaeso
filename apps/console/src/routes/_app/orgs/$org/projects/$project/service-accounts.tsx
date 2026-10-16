import { createFileRoute } from "@tanstack/react-router";
import { serviceAccountsQuery } from "~/api/queries/members";
import { ServiceAccountsPage, type ServiceAccountsSearch } from "~/features/project/service-accounts/service-accounts-page";

const serviceAccountsSearch = (s: Record<string, unknown>): ServiceAccountsSearch => ({
  ...(typeof s.account === "string" ? { account: s.account } : {}),
  ...(s.archived === true || s.archived === "true" ? { archived: true } : {}),
  ...(s.create === true || s.create === "true" ? { create: true } : {}),
});

export const Route = createFileRoute("/_app/orgs/$org/projects/$project/service-accounts")({
  validateSearch: serviceAccountsSearch,
  loaderDeps: ({ search }) => ({ archived: !!serviceAccountsSearch(search).archived }),
  loader: ({ context, params, deps }) => context.queryClient.prefetchQuery(serviceAccountsQuery(params.org, params.project, deps.archived)),
  component: ServiceAccounts,
});

function ServiceAccounts() {
  const { org, project } = Route.useParams();
  return <ServiceAccountsPage org={org} project={project} search={Route.useSearch({ select: serviceAccountsSearch, structuralSharing: true })} />;
}
