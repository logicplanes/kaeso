import { createFileRoute } from "@tanstack/react-router";
import { prefetchAccountSettingsPage } from "~/api/queries/settings-prefetch";

export const Route = createFileRoute("/_app/settings/account")({
  // Runs for every page under it, on a hover or touch of a link to one as well, so it starts that page's data.
  loader: ({ context, location }) => prefetchAccountSettingsPage(context.queryClient, location.pathname),
});
