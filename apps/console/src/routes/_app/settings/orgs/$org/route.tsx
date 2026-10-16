import { createFileRoute, Outlet } from "@tanstack/react-router";
import { sessionQuery } from "~/auth/session";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/settings/orgs/$org")({
  loader: ({ context }) => context.queryClient.ensureQueryData(sessionQuery),
  component: Outlet,
  errorComponent: RouteError,
});
