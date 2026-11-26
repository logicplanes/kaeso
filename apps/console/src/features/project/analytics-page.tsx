import { Suspense, lazy } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useProjectRights } from "~/auth/rights";
import { AppFrame, Tabs } from "@logicplanes/kaeso-ui";
import { spansYears } from "~/format/buckets";
import type { PeriodSearch } from "~/format/period";
import { presetDays } from "~/format/presets";
import { ControlRoom } from "~/kaeso/overview/control-room";
import { useOverview } from "~/kaeso/overview/data";
import type { Dimension } from "~/kaeso/overview/metrics";
import { PROJECT_VIEWS, type AnalyticsSearch, type ProjectView } from "~/kaeso/overview/views";
import { DimensionView } from "~/kaeso/overview/dimension";
import { Efficiency, Reliability } from "~/kaeso/overview/sections";
import { viewTitle } from "~/kaeso/overview/view-title";
import { ProjectHeader } from "~/shell/project-header";
import { useQuery } from "@tanstack/react-query";
import { usageSpanQuery } from "~/api/queries/usage";
import { projectUsageQuery, timelineQuery } from "~/api/queries/usage";
import { clientUsage } from "~/format/clients";
import { shiftDay } from "~/format/period";
import { t } from "~/i18n";
import { PeriodPicker } from "~/kaeso/period-picker";
import { Share } from "~/kaeso/share";

const loadParts = () => import("./analytics-breakdown");
const Breakdown = lazy(() => loadParts().then((m) => ({ default: m.Breakdown })));

const DIM: Partial<Record<ProjectView, Dimension>> = { models: "model", keys: "key", members: "member" };

export function AnalyticsPage({ org, project, search, view }: { org: string; project: string; search: AnalyticsSearch; view?: ProjectView | undefined }) {
  const navigate = useNavigate();
  const o = useOverview({ org, project }, search);
  // By client reads the hourly timeline, which covers at most 62 days of the request log.
  const rights = useProjectRights(org, project);
  const by = (search.by === "client" && !o.hourly) || (search.by === "source" && !rights.has("sources.view")) ? "day" : (search.by ?? "day");
  const span = useQuery(usageSpanQuery(org, project));
  const routes = useQuery({ ...projectUsageQuery(org, project, o.from, o.to, "route"), enabled: by === "route" });
  const sources = useQuery({ ...projectUsageQuery(org, project, o.from, o.to, "source"), enabled: by === "source" });
  const clients = useQuery({
    ...timelineQuery(org, project, `${o.from}T00:00:00Z`, `${shiftDay(o.to, 1)}T00:00:00Z`, "hour", { group_by: "client" }),
    enabled: by === "client",
    select: clientUsage,
  });
  const setSearch = (next: AnalyticsSearch) => void navigate({ to: ".", search: { ...search, ...next }, replace: true });
  const period: PeriodSearch = {
    ...(search.period ? { period: search.period } : {}),
    ...(search.from ? { from: search.from } : {}),
    ...(search.to ? { to: search.to } : {}),
  };
  const kept = { ...period, ...(search.by ? { by: search.by } : {}) };
  const goTo = (next: string) =>
    void navigate(
      next === "overview"
        ? { to: "/orgs/$org/projects/$project/analytics", params: { org, project }, search: kept }
        : { to: "/orgs/$org/projects/$project/analytics/$view", params: { org, project, view: next }, search: kept },
    );
  const tabs = (
    <Tabs variant="pill" value={view ?? "overview"} onValueChange={goTo}>
      <Tabs.List aria-label={t("project.analytics.views", "Analytics views")}>
        <Tabs.Tab value="overview">{t("project.analytics.overview", "Overview")}</Tabs.Tab>
        {PROJECT_VIEWS.map((v) => (
          <Tabs.Tab key={v} value={v}>
            {viewTitle(v, DIM[v], o)}
          </Tabs.Tab>
        ))}
      </Tabs.List>
    </Tabs>
  );
  const table =
    by === "day" ? o.daily : by === "route" ? routes : by === "client" ? clients : by === "source" ? sources : (o.grouped[by as Dimension] ?? o.daily);

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.analytics.title", "Analytics")}
        tabs={tabs}
        actions={
          <>
            <PeriodPicker from={o.from} to={o.to} period={o.period} days={(id) => presetDays(id, new Date(), span.data?.first_day)} onChange={setSearch} />
            <Share compact org={org} target={{ kind: "analytics", project, from: o.from, to: o.to, costs: o.showCost }} />
          </>
        }
      />
      <AppFrame.Body padding="page">
        <div className="flex flex-col">
          {view === "reliability" ? (
            <Reliability o={o} />
          ) : view === "efficiency" ? (
            <Efficiency o={o} />
          ) : view === "breakdown" ? (
            <Suspense fallback={null}>
              <Breakdown
                rows={table.data ?? []}
                by={by}
                granularity={o.g}
                withYear={spansYears(o.from, o.to, o.g)}
                showCost={o.showCost}
                org={org}
                project={project}
                from={o.from}
                to={o.to}
                clients={o.hourly}
                modelName={o.modelName}
                loading={table.isPending || (by === "model" && o.namesLoading)}
                failed={table.isError}
                onRetry={() => void table.refetch()}
                onByChange={(next) => setSearch({ by: next })}
              />
            </Suspense>
          ) : view && DIM[view] ? (
            <DimensionView o={o} dim={DIM[view]} />
          ) : (
            <ControlRoom o={o} {...(search.metric ? { initial: search.metric } : {})} />
          )}
        </div>
      </AppFrame.Body>
    </>
  );
}
