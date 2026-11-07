import { lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Empty, Tabs } from "@logicplanes/kaeso-ui";
import { orgProjectsQuery } from "~/api/queries/projects";
import { usageSpanQuery } from "~/api/queries/usage";
import type { PeriodSearch } from "~/format/period";
import { presetDays } from "~/format/presets";
import { ControlRoom } from "~/kaeso/overview/control-room";
import { useOverview } from "~/kaeso/overview/data";
import type { Dimension } from "~/kaeso/overview/metrics";
import { ORG_VIEWS, type OrgAnalyticsSearch, type OrgView } from "~/kaeso/overview/views";
import { DimensionView, dimensionTitle } from "~/kaeso/overview/dimension";
import { OrgHeader } from "~/shell/org-header";
import { t } from "~/i18n";

const OrgUsageBreakdown = lazy(() => import("./org-usage-breakdown").then((m) => ({ default: m.OrgUsageBreakdown })));
const PeriodPicker = lazy(() => import("~/kaeso/period-picker").then((m) => ({ default: m.PeriodPicker })));

const DIM: Record<OrgView, Dimension> = { projects: "project", models: "model", members: "member" };

export function OrgAnalyticsPage({ org, search, view }: { org: string; search: OrgAnalyticsSearch; view?: OrgView | undefined }) {
  const navigate = useNavigate();
  const by = search.by ?? "project";
  const o = useOverview({ org }, search);
  const span = useQuery({ ...usageSpanQuery(org), enabled: o.canUsage });
  const projects = useQuery(orgProjectsQuery(org, true));
  const ready = o.canUsage && (o.period !== "all" || !span.isPending);
  const setSearch = (next: OrgAnalyticsSearch) => void navigate({ to: ".", search: { ...search, ...next }, replace: true });
  const period: PeriodSearch = {
    ...(search.period ? { period: search.period } : {}),
    ...(search.from ? { from: search.from } : {}),
    ...(search.to ? { to: search.to } : {}),
  };
  const goTo = (next: string) =>
    void navigate(
      next === "overview"
        ? { to: "/orgs/$org/analytics", params: { org }, search: period }
        : { to: "/orgs/$org/analytics/$view", params: { org, view: next }, search: period },
    );
  const tabs = o.canUsage ? (
    <Tabs variant="pill" value={view ?? "overview"} onValueChange={goTo}>
      <Tabs.List aria-label={t("project.analytics.views", "Analytics views")}>
        <Tabs.Tab value="overview">{t("project.analytics.overview", "Overview")}</Tabs.Tab>
        {ORG_VIEWS.map((v) => (
          <Tabs.Tab key={v} value={v}>
            {dimensionTitle(DIM[v])}
          </Tabs.Tab>
        ))}
      </Tabs.List>
    </Tabs>
  ) : undefined;

  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.analytics", "Analytics")}
        {...(tabs ? { tabs } : {})}
        actions={
          <Suspense fallback={null}>
            <PeriodPicker from={o.from} to={o.to} period={o.period} days={(id) => presetDays(id, new Date(), span.data?.first_day)} onChange={setSearch} />
          </Suspense>
        }
      />
      <AppFrame.Body padding="page">
        <div className="flex flex-col">
          {!o.canUsage ? (
            <Empty
              size="md"
              title={t("orgHome.noUsage", "You can't see this organization's usage")}
              description={t("orgHome.noUsageHint", "An owner can give you the right to view it.")}
            />
          ) : view ? (
            <DimensionView o={o} dim={DIM[view]} />
          ) : (
            <>
              <ControlRoom o={o} {...(search.metric ? { initial: search.metric } : {})} />
              <Suspense fallback={null}>
                <OrgUsageBreakdown
                  org={org}
                  from={o.from}
                  to={o.to}
                  by={by}
                  onByChange={(b) => setSearch({ by: b })}
                  projects={projects.data ?? (projects.isError ? [] : undefined)}
                  showCost={o.showCost}
                  ready={ready}
                />
              </Suspense>
            </>
          )}
        </div>
      </AppFrame.Body>
    </>
  );
}
