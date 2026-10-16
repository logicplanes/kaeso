import { createFileRoute, Outlet } from "@tanstack/react-router";
import { OrgGuard } from "~/auth/guards";
import { writeLast } from "~/shell/last";
import { RouteError } from "~/shell/route-error";

const IN_PROJECT = /^\/orgs\/[^/]+\/projects\/[^/]+/;

export const Route = createFileRoute("/_app/orgs/$org")({
  loader: ({ params, location, preload }) => {
    if (!preload && !IN_PROJECT.test(location.pathname)) writeLast({ org: params.org });
  },
  component: OrgLayout,
  errorComponent: RouteError,
});

function OrgLayout() {
  return (
    <OrgGuard org={Route.useParams().org}>
      <Outlet />
    </OrgGuard>
  );
}
