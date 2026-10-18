import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/instance/")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/instance/general", replace: true });
  },
});
