import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/orgs/$org/invites")({
  validateSearch: (s: Record<string, unknown>): { invite?: boolean } => (s.invite === true || s.invite === "true" ? { invite: true } : {}),
  beforeLoad: ({ params, search }) => {
    throw redirect({ to: "/settings/orgs/$org/members", params: { org: params.org }, search: { tab: "invites", ...search }, replace: true });
  },
});
