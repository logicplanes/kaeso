import { createFileRoute, redirect } from "@tanstack/react-router";
import type { OrgProjectsSearch } from "~/features/org-home/org-projects-page";

export const Route = createFileRoute("/_app/settings/orgs/$org/projects")({
  validateSearch: (s: Record<string, unknown>): OrgProjectsSearch => (s.create === true || s.create === "true" ? { create: true } : {}),
  beforeLoad: ({ params, search }) => {
    throw redirect({ to: "/orgs/$org/projects", params: { org: params.org }, search, replace: true });
  },
});
