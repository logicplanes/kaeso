import { createFileRoute, redirect } from "@tanstack/react-router";
import { isApiError } from "~/api/errors";
import { membershipsQuery, sessionQuery } from "~/auth/session";
import { loadRouteGates } from "~/auth/route-gates-load";
import { AppShell } from "~/shell/app-shell";
import { GatewayUnreachable } from "~/shell/gateway-unreachable";
import { SessionPending } from "~/shell/pending";

export const Route = createFileRoute("/_app")({
  beforeLoad: async ({ context, location }) => {
    loadRouteGates().catch(() => undefined);
    try {
      await context.queryClient.ensureQueryData(sessionQuery);
      await context.queryClient.ensureQueryData(membershipsQuery);
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
