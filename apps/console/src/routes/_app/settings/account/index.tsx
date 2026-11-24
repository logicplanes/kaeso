import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/settings/account/")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/account/profile", replace: true });
  },
});
