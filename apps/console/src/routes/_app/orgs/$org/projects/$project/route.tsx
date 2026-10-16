import { createFileRoute } from "@tanstack/react-router";
import { loadRouteGates } from "~/auth/route-gates-load";
import { PagePending } from "~/shell/page-pending";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/orgs/$org/projects/$project")({
  beforeLoad: async (ctx) => (await loadRouteGates()).projectGate(ctx),
  pendingComponent: PagePending,
  errorComponent: RouteError,
});
