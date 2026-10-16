import { createFileRoute } from "@tanstack/react-router";
import { loadRouteGates } from "~/auth/route-gates-load";
import { NoMemberships } from "~/shell/no-memberships";

export const Route = createFileRoute("/_app/")({
  beforeLoad: async ({ context }) => (await loadRouteGates()).goHome(context.queryClient),
  component: NoMemberships,
});
