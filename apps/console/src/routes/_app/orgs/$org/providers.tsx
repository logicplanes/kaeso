import { createFileRoute } from "@tanstack/react-router";
import { OrgProvidersPage, type ProvidersSearch } from "~/features/org-home/providers-page";

const providersSearch = (s: Record<string, unknown>): ProvidersSearch => (s.tab === "custom" ? { tab: "custom" } : {});

export const Route = createFileRoute("/_app/orgs/$org/providers")({ validateSearch: providersSearch, component: Providers });

function Providers() {
  return <OrgProvidersPage org={Route.useParams().org} search={Route.useSearch({ select: providersSearch, structuralSharing: true })} />;
}
