import { lazy } from "react";
import { lazyReady } from "~/shell/lazy-ready";

// Split from the page for its bundle budget (scripts/budget.mjs). The parts that would move the page draw with it when
// the route's preload brought their code first; the glance at the bottom moves nothing when React reveals it later.
export const ShareCard = lazyReady(() => import("./share-card").then((m) => m.ShareCard));
export const Limit = lazyReady(() => import("~/kaeso/overview/limit").then((m) => m.Limit));
const glance = () => import("./overview-glance");
export const ProjectGlance = lazy(() => glance().then((m) => ({ default: m.ProjectGlance })));

// A part that fails to load here tries again when the page draws it.
export function preloadProjectOverview(): void {
  void Promise.allSettled([ShareCard.preload(), Limit.preload(), glance()]);
}
