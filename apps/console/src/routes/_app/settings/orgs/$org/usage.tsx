import { createFileRoute, redirect } from "@tanstack/react-router";
import type { OrgOverviewSearch } from "~/features/org-home/org-overview-page";
import { periodSearch } from "~/format/period";

export const Route = createFileRoute("/_app/settings/orgs/$org/usage")({
  validateSearch: (s: Record<string, unknown>): OrgOverviewSearch => periodSearch(s),
  beforeLoad: ({ params, search }) => {
    throw redirect({ to: "/orgs/$org", params: { org: params.org }, search, replace: true });
  },
});
