import { useQuery } from "@tanstack/react-query";
import { Suspense } from "react";
import { AppFrame, Button } from "@logicplanes/kaeso-ui";
import { limitsQuery } from "~/api/queries/limits";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { HomeMetrics } from "~/kaeso/overview/home-parts";
import { ProjectHeader } from "~/shell/project-header";
import { projectPath } from "~/shell/pages";
import { Limit, ProjectGlance, ShareCard } from "./overview-parts";

export function OverviewPage({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const base = projectPath(org, project);
  const limits = useQuery({ ...limitsQuery(org, project, { level: "project" }), enabled: rights.has("limits.view") });
  const spend = limits.data?.find((l) => l.kind === "spend_budget");
  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.overview.title", "Overview")}
        actions={
          <>
            <Suspense fallback={null}>
              <ShareCard org={org} project={project} />
            </Suspense>
            <Button size="sm" href={`${base}/analytics`}>
              {t("project.home.openAnalytics", "Open analytics")}
            </Button>
          </>
        }
      />
      <AppFrame.Body padding="page">
        <div className="flex flex-col">
          <HomeMetrics
            org={org}
            project={project}
            canUsage
            showCost={rights.has("costs.view")}
            analytics={`${base}/analytics`}
            monthExtra={
              spend ? (
                <Suspense fallback={null}>
                  <Limit limit={spend} />
                </Suspense>
              ) : undefined
            }
          />
          <Suspense fallback={null}>
            <ProjectGlance org={org} project={project} />
          </Suspense>
        </div>
      </AppFrame.Body>
    </>
  );
}
