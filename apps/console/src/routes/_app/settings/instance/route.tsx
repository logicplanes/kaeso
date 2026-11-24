import { createFileRoute } from "@tanstack/react-router";
import { prefetchInstanceSettingsPage } from "~/api/queries/settings-prefetch";
import { sessionQuery } from "~/auth/session";
import { InstanceGate } from "~/features/instance/instance-page";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/settings/instance")({
  // Runs for every page under the layout, on a hover or touch of a link to one as well, so it starts that page's data.
  loader: async ({ context, location }) => {
    await context.queryClient.ensureQueryData(sessionQuery);
    prefetchInstanceSettingsPage(context.queryClient, location.pathname, location.search);
  },
  component: InstanceGate,
  errorComponent: RouteError,
});
