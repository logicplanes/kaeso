import { createFileRoute, Outlet } from "@tanstack/react-router";
import { sessionQuery } from "~/auth/session";
import { OrgProviderMarks } from "~/kaeso/provider-marks";
import { RouteError } from "~/shell/route-error";

export const Route = createFileRoute("/_app/settings/orgs/$org")({
  loader: ({ context }) => context.queryClient.ensureQueryData(sessionQuery),
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
