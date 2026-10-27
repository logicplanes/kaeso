import { createFileRoute, redirect } from "@tanstack/react-router";
import { isApiError } from "~/api/errors";
import { membershipsQuery, sessionQuery } from "~/auth/session-queries";
import { loadRouteGates } from "~/auth/route-gates-load";
import { AppShell } from "~/shell/app-shell";
import { GatewayUnreachable } from "~/shell/gateway-unreachable";
import { lazyPending } from "~/shell/lazy-pending";

const SessionPending = lazyPending(() => import("~/shell/pending").then((m) => m.SessionPending));

export const Route = createFileRoute("/_app")({
  beforeLoad: async ({ context, location }) => {
    loadRouteGates().catch(() => undefined);
    try {
      const memberships = context.queryClient.ensureQueryData(membershipsQuery);
      memberships.catch(() => undefined);
      await context.queryClient.ensureQueryData(sessionQuery);
      await memberships;
    } catch (error) {
      if (isApiError(error) && error.type === "authentication") {
        void import("~/auth/user-data").then((m) => m.forgetUserData());
        throw redirect({ to: "/sign-in", search: { next: location.href } });
      }
      throw error;
    }
  },
  component: AppShell,
  pendingComponent: SessionPending,
  errorComponent: GatewayUnreachable,
});
