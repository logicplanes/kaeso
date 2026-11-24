import { createFileRoute, Outlet } from "@tanstack/react-router";
import { prefetchOrgSettingsPage } from "~/api/queries/settings-prefetch";
import { sessionQuery } from "~/auth/session";
import { OrgProviderMarks } from "~/kaeso/provider-marks";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/settings/orgs/$org")({
  // Runs for every page under the layout, on a hover or touch of a link to one as well, so it starts that page's data.
  loader: async ({ context, params, location }) => {
    await context.queryClient.ensureQueryData(sessionQuery);
    prefetchOrgSettingsPage(context.queryClient, params.org, location.pathname, location.search);
  },
  component: OrgSettingsLayout,
  errorComponent: RouteError,
});

function OrgSettingsLayout() {
  return (
    <OrgProviderMarks org={Route.useParams().org}>
      <Outlet />
    </OrgProviderMarks>
  );
}
