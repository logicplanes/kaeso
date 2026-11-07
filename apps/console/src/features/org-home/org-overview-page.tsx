import { lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppFrame, Button } from "@logicplanes/kaeso-ui";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { HomeMetrics } from "~/kaeso/overview/home-parts";
import { OrgHeader } from "~/shell/org-header";
import { orgPath } from "~/shell/pages";

// Split from the page so the overview's first load stays within its bundle budget (scripts/budget.mjs); the route preloads them.
const glance = () => import("./org-glance");
const news = () => import("./home-news");
const OrgGlance = lazy(() => glance().then((m) => ({ default: m.OrgGlance })));
const CostByProject = lazy(() => glance().then((m) => ({ default: m.CostByProject })));
const ReleaseBanner = lazy(() => news().then((m) => ({ default: m.ReleaseBanner })));
const NewModels = lazy(() => news().then((m) => ({ default: m.NewModels })));

export function OrgOverviewPage({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const canUsage = rights.has("usage.org.view");
  const showCost = rights.has("costs.org.view");
  const projects = useQuery(orgProjectsQuery(org, true));
  const first = projects.data?.find((p) => !p.archived_at);
  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.overview", "Overview")}
        actions={
          canUsage && (
            <Button size="sm" href={`${orgPath(org)}/analytics`}>
              {t("project.home.openAnalytics", "Open analytics")}
            </Button>
          )
        }
      />
      <AppFrame.Body padding="page">
        <div className="flex flex-col">
          <Suspense fallback={null}>
            <ReleaseBanner />
          </Suspense>
          <div className="mt-48">
            <HomeMetrics
              org={org}
              canUsage={canUsage}
              showCost={showCost}
              analytics={`${orgPath(org)}/analytics`}
              monthExtra={
                showCost ? (
                  <Suspense fallback={null}>
                    <CostByProject org={org} />
                  </Suspense>
                ) : undefined
              }
            />
          </div>
          <Suspense fallback={null}>
            <OrgGlance org={org} />
            {first && <NewModels org={org} project={first.id} />}
          </Suspense>
        </div>
      </AppFrame.Body>
    </>
  );
}
