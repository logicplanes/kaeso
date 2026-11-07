import { createFileRoute, redirect } from "@tanstack/react-router";
import type { OrgAnalyticsSearch } from "~/kaeso/overview/views";
import { periodSearch } from "~/format/period-search";

export const Route = createFileRoute("/_app/settings/orgs/$org/usage")({
  validateSearch: (s: Record<string, unknown>): OrgAnalyticsSearch => periodSearch(s),
  beforeLoad: ({ params, search }) => {
    throw redirect({ to: "/orgs/$org/analytics", params: { org: params.org }, search, replace: true });
  },
});
