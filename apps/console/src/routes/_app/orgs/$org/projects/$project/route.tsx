import { createFileRoute } from "@tanstack/react-router";
import { loadRouteGates } from "~/auth/route-gates-load";
import { lazyPending } from "~/shell/lazy-pending";
import { RouteError } from "~/shell/route-error";

const PagePending = lazyPending(() => import("~/shell/page-pending").then((m) => m.PagePending));

export const Route = createFileRoute("/_app/orgs/$org/projects/$project")({
  beforeLoad: async (ctx) => (await loadRouteGates()).projectGate(ctx),
  pendingComponent: PagePending,
  errorComponent: RouteError,
});
