import { lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Badge, Button, List, Section } from "@logicplanes/kaeso-ui";
import { orgProjectsQuery } from "~/api/queries/projects";
import { orgUsageQuery, usageSpanQuery } from "~/api/queries/usage";
import { useOrgRights } from "~/auth/session";
import { granularity } from "~/format/buckets";
import { previousPeriod, type PeriodSearch } from "~/format/period";
import { comparedWith, presetDays, resolvePeriod } from "~/format/presets";
import { PeriodPickerPlaceholder } from "~/kaeso/period-placeholder";
import { usePictureHues } from "~/kaeso/picture-hues";
import { RequestsChart, TokensChart, UsageMetrics } from "~/kaeso/usage-panels";
import { OrgHeader } from "~/shell/org-header";
import { projectPath } from "~/shell/pages";
import type { OrgBreakdown } from "./org-usage-breakdown";
import { t } from "~/i18n";

const CostShare = lazy(() => import("~/kaeso/cost-share").then((m) => ({ default: m.CostShare })));
const OrgUsageBreakdown = lazy(() => import("./org-usage-breakdown").then((m) => ({ default: m.OrgUsageBreakdown })));

const PeriodPicker = lazy(() => import("~/kaeso/period-picker").then((m) => ({ default: m.PeriodPicker })));

export type OrgOverviewSearch = PeriodSearch & { by?: OrgBreakdown };

export function OrgOverviewPage({ org, search }: { org: string; search: OrgOverviewSearch }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const by = search.by ?? "project";
  const canUsage = rights.has("usage.org.view");
  const showCost = rights.has("costs.org.view");
  const span = useQuery({ ...usageSpanQuery(org), enabled: canUsage });
  const { from, to, period } = resolvePeriod(search, span.data?.first_day);
  const ready = canUsage && (period !== "all" || !span.isPending);
  const g = granularity(from, to);
  const daily = useQuery({ ...orgUsageQuery(org, from, to, g), enabled: ready });
  const before = previousPeriod(from, to);
  const compare = comparedWith(period, from, to);
  const previous = useQuery({ ...orgUsageQuery(org, before.from, before.to, g), enabled: ready && !!compare });
  const byProject = useQuery({ ...orgUsageQuery(org, from, to, "project"), enabled: ready });
  const projects = useQuery(orgProjectsQuery(org, true));
  const setSearch = (next: OrgOverviewSearch) => void navigate({ to: "/orgs/$org", params: { org }, search: { ...search, ...next }, replace: true });
  const dailyState = { rows: daily.data ?? [], from, to, granularity: g, loading: daily.isPending, failed: daily.isError, onRetry: () => void daily.refetch() };
  const name = (id: string) => projects.data?.find((p) => p.id === id)?.name ?? id;
  const hues = usePictureHues((projects.data ?? []).map((p) => ({ id: p.id, src: p.avatar_url })));

  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.overview", "Overview")}
        actions={
          canUsage && (
            <Suspense fallback={<PeriodPickerPlaceholder />}>
              <PeriodPicker from={from} to={to} period={period} days={(id) => presetDays(id, new Date(), span.data?.first_day)} onChange={setSearch} />
            </Suspense>
          )
        }
      />
      <AppFrame.Body padding="page">
        <div key={`${from}/${to}/${ready}`} className="flex flex-col">
          {canUsage && (
            <>
              <UsageMetrics
                rows={daily.data ?? []}
                previous={previous.data}
                from={from}
                to={to}
                granularity={g}
                compare={compare}
                showCost={showCost}
                loading={daily.isPending || daily.isError || previous.isLoading}
              />
              <RequestsChart {...dailyState} />
              <TokensChart {...dailyState} />
              {showCost && (
                <Suspense fallback={null}>
                  <CostShare
                    title={t("orgHome.costByProject", "Cost by project")}
                    description={t("orgHome.costByProjectHint", "USD in the period")}
                    rows={byProject.data ?? []}
                    label={name}
                    hues={hues}
                    loading={byProject.isPending || projects.isPending}
                    failed={byProject.isError}
                    onRetry={() => void byProject.refetch()}
                  />
                </Suspense>
              )}
              <Suspense fallback={null}>
                <OrgUsageBreakdown
                  org={org}
                  from={from}
                  to={to}
                  by={by}
                  onByChange={(b) => setSearch({ by: b })}
                  projects={projects.data ?? (projects.isError ? [] : undefined)}
                  showCost={showCost}
                  ready={ready}
                />
              </Suspense>
            </>
          )}
          {!canUsage && (
            <Section
              title={t("common.projects", "Projects")}
              description={t("orgHome.projectsHint", "Open one to work in it")}
              actions={
                <Button size="sm" href={`/orgs/${org}/projects`}>
                  {t("orgHome.allProjects", "All projects")}
                </Button>
              }
            >
              <List
                aria-label={t("common.projects", "Projects")}
                loading={projects.isPending}
                items={(projects.data ?? [])
                  .filter((p) => !p.archived_at)
                  .map((p) => ({
                    id: p.id,
                    title: p.name,
                    leading: <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />,
                    meta: p.is_default ? <Badge>{t("orgHome.default", "Default")}</Badge> : undefined,
                  }))}
                onOpen={(item) => void navigate({ href: projectPath(org, item.id) })}
              />
            </Section>
          )}
        </div>
      </AppFrame.Body>
    </>
  );
}
