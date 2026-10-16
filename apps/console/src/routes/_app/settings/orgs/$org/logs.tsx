import { createFileRoute, redirect } from "@tanstack/react-router";
import type { LoggingTab } from "~/api/queries/settings-prefetch";

export const Route = createFileRoute("/_app/settings/orgs/$org/logs")({
  validateSearch: (s: Record<string, unknown>): { tab?: LoggingTab } => (s.tab === "storage" || s.tab === "deletions" ? { tab: s.tab } : {}),
  beforeLoad: ({ params, search }) => {
    throw redirect({ to: "/settings/orgs/$org/logging", params: { org: params.org }, search, replace: true });
  },
});
