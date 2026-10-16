import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/instance/models")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/instance/model-sources", replace: true });
  },
});
