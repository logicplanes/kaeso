import { Suspense, lazy } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, SplitView } from "@logicplanes/kaeso-ui";
import { catalogueQuery } from "~/api/queries/models";
import { projectUsageQuery, usageSpanQuery, type GroupBy } from "~/api/queries/usage";
import { granularity } from "~/format/buckets";
import { previousPeriod, type PeriodSearch } from "~/format/period";
import { comparedWith, presetDays, resolvePeriod } from "~/format/presets";
import { CostChart, RequestsChart, TokensChart, UsageMetrics } from "~/kaeso/usage-panels";
import { ProjectHeader } from "~/shell/project-header";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";

const loadParts = () => import("./overview-parts");
const Breakdown = lazy(() => loadParts().then((m) => ({ default: m.Breakdown })));
const Aside = lazy(() => loadParts().then((m) => ({ default: m.Aside })));
const CostShare = lazy(() => loadParts().then((m) => ({ default: m.CostShare })));
const PeriodPicker = lazy(() => import("~/kaeso/period-picker").then((m) => ({ default: m.PeriodPicker })));

export type OverviewSearch = PeriodSearch & { by?: Exclude<GroupBy, "project" | "week" | "month"> };

export function OverviewPage({ org, project, search }: { org: string; project: string; search: OverviewSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const by = search.by ?? "model";
  const showCost = rights.has("costs.view");

  const span = useQuery(usageSpanQuery(org, project));
  const { from, to, period } = resolvePeriod(search, span.data?.first_day);
  const ready = period !== "all" || !span.isPending;
  const g = granularity(from, to);
  const daily = useQuery({ ...projectUsageQuery(org, project, from, to, g), enabled: ready });
  const before = previousPeriod(from, to);
  const compare = comparedWith(period, before.days);
  const previous = useQuery({ ...projectUsageQuery(org, project, before.from, before.to, g), enabled: ready && !!compare });
  const grouped = useQuery({ ...projectUsageQuery(org, project, from, to, by), enabled: ready && by !== "day" });
  const byModel = useQuery({ ...projectUsageQuery(org, project, from, to, "model"), enabled: ready && showCost });
  const models = useQuery({ ...catalogueQuery(org, project), enabled: rights.has("models.view") });
  const naming = models.isLoading;
  const modelName = (key: string) => {
    const id = key.split("/").slice(1).join("/");
    if (!id) return t("project.overview.noModel", "No model");
    return models.data?.find((m) => m.model_id === id && key.startsWith(`${m.provider}/`))?.effective.display_name || id;
  };

  const setSearch = (next: OverviewSearch) =>
    void navigate({ to: "/orgs/$org/projects/$project", params: { org, project }, search: { ...search, ...next }, replace: true });
  const dailyState = { rows: daily.data ?? [], from, to, granularity: g, loading: daily.isPending, failed: daily.isError, onRetry: () => void daily.refetch() };

  const tableRows = by === "day" ? (daily.data ?? []) : (grouped.data ?? []);

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.overview.title", "Overview")}
        actions={
          <Suspense fallback={null}>
            <PeriodPicker from={from} to={to} period={period} days={(id) => presetDays(id, new Date(), span.data?.first_day)} onChange={setSearch} />
          </Suspense>
        }
      />
      <AppFrame.Body padding="page">
        <SplitView
          aside={
            <Suspense fallback={null}>
              <Aside org={org} project={project} modelCount={models.data?.length} />
            </Suspense>
          }
        >
          <div key={`${from}/${to}/${ready}`} className="flex flex-col">
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
            {showCost && <CostChart {...dailyState} />}
            <Suspense fallback={null}>
              {showCost && (
                <CostShare
                  title={t("project.overview.costByModel", "Cost by model")}
                  description={t("project.overview.costShare", "Share of the period's cost")}
                  rows={byModel.data ?? []}
                  label={modelName}
                  loading={byModel.isPending || naming}
                  failed={byModel.isError}
                  onRetry={() => void byModel.refetch()}
                />
              )}
              <Breakdown
                rows={tableRows}
                by={by}
                granularity={g}
                showCost={showCost}
                org={org}
                project={project}
                modelName={modelName}
                loading={by === "day" ? daily.isPending : grouped.isPending || (by === "model" && naming)}
                failed={by === "day" ? daily.isError : grouped.isError}
                onRetry={() => void (by === "day" ? daily.refetch() : grouped.refetch())}
                onByChange={(next) => setSearch({ by: next })}
              />
            </Suspense>
          </div>
        </SplitView>
      </AppFrame.Body>
    </>
  );
}
