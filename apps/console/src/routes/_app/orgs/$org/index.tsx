import { createFileRoute } from "@tanstack/react-router";
import { OrgOverviewPage, type OrgOverviewSearch } from "~/features/org-home/org-overview-page";
import { periodSearch } from "~/format/period";

const BY = ["project", "model", "member"] as const;

function overviewSearch(s: Record<string, unknown>): OrgOverviewSearch {
  const by = BY.find((b) => b === s.by);
  return { ...periodSearch(s), ...(by ? { by } : {}) };
}

export const Route = createFileRoute("/_app/orgs/$org/")({ validateSearch: overviewSearch, component: Overview });

function Overview() {
  return <OrgOverviewPage org={Route.useParams().org} search={Route.useSearch({ select: overviewSearch, structuralSharing: true })} />;
}
