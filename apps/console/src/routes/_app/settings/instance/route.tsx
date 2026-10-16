import { createFileRoute } from "@tanstack/react-router";
import { sessionQuery } from "~/auth/session";
import { InstanceGate } from "~/features/instance/instance-page";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/settings/instance")({
  loader: ({ context }) => context.queryClient.ensureQueryData(sessionQuery),
  component: InstanceGate,
  errorComponent: RouteError,
});
