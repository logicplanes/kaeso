import { createRootRouteWithContext, Outlet } from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import { useLinkVisit } from "~/auth/fragment";

export interface RouterContext {
  queryClient: QueryClient;
}

function Root() {
  return <Outlet key={useLinkVisit()} />;
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: Root,
});
