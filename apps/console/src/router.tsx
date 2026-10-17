import { createRouter } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";
import { queryClient } from "./query";

export function createAppRouter() {
  return createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: "intent",
    defaultPreloadStaleTime: 0,
    scrollRestoration: () => false,
    defaultRemountDeps: ({ params }) => params,
  });
}

type AppRouter = ReturnType<typeof createAppRouter>;

export function loadPageChunks(router: AppRouter, pathname: string): Promise<unknown> {
  const [branch] = router.getMatchedRoutes(pathname);
  return Promise.allSettled(branch.map((route) => router.loadRouteChunk(route)));
}

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}
