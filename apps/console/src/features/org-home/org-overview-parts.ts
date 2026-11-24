import { lazy } from "react";
import { lazyReady } from "~/shell/lazy-ready";

// Split from the page for its bundle budget (scripts/budget.mjs). The parts that would move the page draw with it when
// the route's preload brought their code first; the sections at the bottom move nothing when React reveals them later.
export const ReleaseBanner = lazyReady(() => import("./release-banner").then((m) => m.ReleaseBanner));
// The route prefetches the retiring-models warning's data too, so on a visit whose data is in it draws above the
// activity in the page's first frame; in the page's own code it would take 3 kB of the first load.
export const RetiringProjects = lazyReady(() => import("./retiring-projects").then((m) => m.RetiringProjects));
export const CostByProject = lazyReady(() => import("./cost-by-project").then((m) => m.CostByProject));
const glance = () => import("./org-glance");
const news = () => import("./home-news");
export const OrgGlance = lazy(() => glance().then((m) => ({ default: m.OrgGlance })));
export const NewModels = lazy(() => news().then((m) => ({ default: m.NewModels })));

// A part that fails to load here tries again when the page draws it.
export function preloadOrgOverview(): void {
  void Promise.allSettled([ReleaseBanner.preload(), RetiringProjects.preload(), CostByProject.preload(), glance(), news()]);
}
