import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/orgs/$org/")({
  beforeLoad: ({ params }) => {
    throw redirect({ to: "/settings/orgs/$org/general", params, replace: true });
  },
});
