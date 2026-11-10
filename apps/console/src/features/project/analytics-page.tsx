import { Suspense, lazy } from "react";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Tabs } from "@logicplanes/kaeso-ui";
import { spansYears } from "~/format/buckets";
import type { PeriodSearch } from "~/format/period";
import { presetDays } from "~/format/presets";
import { ControlRoom } from "~/kaeso/overview/control-room";
import { useOverview } from "~/kaeso/overview/data";
import type { Dimension } from "~/kaeso/overview/metrics";
import { PROJECT_VIEWS, type AnalyticsSearch, type ProjectView } from "~/kaeso/overview/views";
import { DimensionView, dimensionTitle } from "~/kaeso/overview/dimension";
import { ProjectHeader } from "~/shell/project-header";
import { useQuery } from "@tanstack/react-query";
import { usageSpanQuery } from "~/api/queries/usage";
import { projectUsageQuery } from "~/api/queries/usage";
import { t } from "~/i18n";

const loadParts = () => import("./analytics-breakdown");
const Breakdown = lazy(() => loadParts().then((m) => ({ default: m.Breakdown })));
const PeriodPicker = lazy(() => import("~/kaeso/period-picker").then((m) => ({ default: m.PeriodPicker })));

const DIM: Record<ProjectView, Dimension> = { models: "model", keys: "key", members: "member" };

export function AnalyticsPage({ org, project, search, view }: { org: string; project: string; search: AnalyticsSearch; view?: ProjectView | undefined }) {
  const navigate = useNavigate();
  const by = search.by ?? "day";
  const o = useOverview({ org, project }, search);
  const span = useQuery(usageSpanQuery(org, project));
  const routes = useQuery({ ...projectUsageQuery(org, project, o.from, o.to, "route"), enabled: by === "route" });
  const setSearch = (next: AnalyticsSearch) => void navigate({ to: ".", search: { ...search, ...next }, replace: true });
  const period: PeriodSearch = {
    ...(search.period ? { period: search.period } : {}),
    ...(search.from ? { from: search.from } : {}),
    ...(search.to ? { to: search.to } : {}),
  };
  const goTo = (next: string) =>
    void navigate(
      next === "overview"
        ? { to: "/orgs/$org/projects/$project/analytics", params: { org, project }, search: period }
        : { to: "/orgs/$org/projects/$project/analytics/$view", params: { org, project, view: next }, search: period },
    );
  const tabs = (
    <Tabs variant="pill" value={view ?? "overview"} onValueChange={goTo}>
      <Tabs.List aria-label={t("project.analytics.views", "Analytics views")}>
        <Tabs.Tab value="overview">{t("project.analytics.overview", "Overview")}</Tabs.Tab>
        {PROJECT_VIEWS.map((v) => (
          <Tabs.Tab key={v} value={v}>
            {dimensionTitle(DIM[v])}
          </Tabs.Tab>
        ))}
      </Tabs.List>
    </Tabs>
  );
  const table = by === "day" ? o.daily : by === "route" ? routes : (o.grouped[by] ?? o.daily);

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.analytics.title", "Analytics")}
        tabs={tabs}
        actions={
          <Suspense fallback={null}>
            <PeriodPicker from={o.from} to={o.to} period={o.period} days={(id) => presetDays(id, new Date(), span.data?.first_day)} onChange={setSearch} />
          </Suspense>
        }
      />
      <AppFrame.Body padding="page">
        <div className="flex flex-col">
          {view ? (
            <DimensionView o={o} dim={DIM[view]} />
          ) : (
            <>
              <ControlRoom o={o} {...(search.metric ? { initial: search.metric } : {})} />
              <Suspense fallback={null}>
                <Breakdown
                  rows={table.data ?? []}
                  by={by}
                  granularity={o.g}
                  withYear={spansYears(o.from, o.to, o.g)}
                  showCost={o.showCost}
                  org={org}
                  project={project}
                  modelName={o.modelName}
                  loading={table.isPending || (by === "model" && o.namesLoading)}
                  failed={table.isError}
                  onRetry={() => void table.refetch()}
                  onByChange={(next) => setSearch({ by: next })}
                />
              </Suspense>
            </>
          )}
        </div>
      </AppFrame.Body>
    </>
  );
}
