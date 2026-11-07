import { useQuery, type QueryKey } from "@tanstack/react-query";
import type { UsageRow } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { limitsQuery } from "~/api/queries/limits";
import { catalogueQuery } from "~/api/queries/models";
import { orgMembersQuery } from "~/api/queries/orgs";
import { orgProjectsQuery } from "~/api/queries/projects";
import { recentErrorsQuery } from "~/api/queries/recent-errors";
import { orgUsageQuery, projectUsageQuery, timelineQuery, usageSpanQuery, type GroupBy, type TimelineStep } from "~/api/queries/usage";
import { useProjectRights } from "~/auth/rights";
import { useOrgRights, useRequestLogs, useSession } from "~/auth/session";
import { analyticsGranularity } from "~/format/buckets";
import { previousPeriod, shiftDay, utcDay, type PeriodSearch } from "~/format/period";
import { comparedWith, resolvePeriod } from "~/format/presets";
import { t } from "~/i18n";
import { useProviders } from "~/kaeso/providers";
import { byBucket, totals } from "~/kaeso/usage-panels";
import { keyLabel, serviceAccountName, usePartyName } from "~/features/project/keys/party";
import { forecast, type Dimension } from "./metrics";
import { fill, iso, useMinute } from "./timeline";

/** Keeps the last period's rows on screen while the next period loads, but only for the same scope and grouping, so another project's numbers never show through. */
function keepWithin(key: QueryKey, by: GroupBy) {
  const scope = JSON.stringify(key.slice(0, -1));
  return (previous: UsageRow[] | undefined, query: { queryKey: QueryKey } | undefined): UsageRow[] | undefined => {
    if (!query) return undefined;
    const last = query.queryKey.at(-1) as { groupBy?: string } | undefined;
    return JSON.stringify(query.queryKey.slice(0, -1)) === scope && last?.groupBy === by ? previous : undefined;
  };
}

export interface Scope {
  org: string;
  project?: string | undefined;
}

export function useOverview(scope: Scope, search: PeriodSearch) {
  const { org, project } = scope;
  const session = useSession();
  const orgRights = useOrgRights(org);
  const projectRights = useProjectRights(org, project ?? "");
  const can = (right: string) => projectRights.has(right);
  const canUsage = project ? true : orgRights.has("usage.org.view");
  const showCost = project ? projectRights.has("costs.view") : orgRights.has("costs.org.view");

  const logsOn = useRequestLogs();
  const span = useQuery({ ...usageSpanQuery(org, project), enabled: canUsage });
  const { from, to, period } = resolvePeriod(search, span.data?.first_day);
  const ready = canUsage && (period !== "all" || !span.isPending);
  const g = analyticsGranularity(from, to);
  const before = previousPeriod(from, to);
  const compare = comparedWith(period, from, to);
  const base = (f: string, tt: string, by: GroupBy) => (project ? projectUsageQuery(org, project, f, tt, by) : orgUsageQuery(org, f, tt, by));
  const usage = (f: string, tt: string, by: GroupBy) => {
    const q = base(f, tt, by);
    return { ...q, placeholderData: keepWithin(q.queryKey, by) };
  };

  const daily = useQuery({ ...usage(from, to, g), enabled: ready });
  const nowMs = useMinute();
  const startMs = Date.parse(`${from}T00:00:00Z`);
  const endMs = Math.min(Date.parse(`${to}T00:00:00Z`) + 86_400_000, nowMs + 60_000);
  const periodMs = Date.parse(`${to}T00:00:00Z`) + 86_400_000 - startMs;
  const step: TimelineStep | undefined = !logsOn ? undefined : periodMs <= 86_400_000 ? "minute" : periodMs <= 31 * 86_400_000 ? "hour" : undefined;
  const fine = useQuery({
    ...timelineQuery(org, project, iso(startMs), iso(endMs), step ?? "hour"),
    enabled: ready && !!step,
    placeholderData: (p) => p,
    retry: false,
  });
  const fineStep = fine.isError ? undefined : step;
  const finePrev = useQuery({
    ...timelineQuery(org, project, iso(startMs - periodMs), iso(endMs - periodMs), step ?? "hour"),
    enabled: ready && !!step && !!compare,
    placeholderData: (p) => p,
  });
  const single = from === to;
  const contextFrom = shiftDay(to, -13);
  const context = useQuery({ ...usage(contextFrom, to, "day"), enabled: ready && single });
  const previous = useQuery({ ...usage(before.from, before.to, g), enabled: ready && !!compare });
  const [first, second, third]: [Dimension, Dimension, Dimension] = project ? ["model", "key", "member"] : ["model", "project", "member"];
  const byFirst = useQuery({ ...usage(from, to, first), enabled: ready });
  const bySecond = useQuery({ ...usage(from, to, second), enabled: ready });
  const byThird = useQuery({ ...usage(from, to, third), enabled: ready });
  const grouped: Partial<Record<Dimension, typeof byFirst>> = { [first]: byFirst, [second]: bySecond, [third]: byThird };

  const models = useQuery({ ...catalogueQuery(org, project ?? ""), enabled: !!project && can("models.view") });
  const keys = useQuery({ ...allKeysQuery(org, project ?? "", { include_archived: true }), enabled: !!project && can("keys.all.view") });
  const projects = useQuery({ ...orgProjectsQuery(org, true), enabled: !project });
  const orgMembers = useQuery({ ...orgMembersQuery(org), enabled: !project && orgRights.has("members.view") });
  const limits = useQuery({ ...limitsQuery(org, project ?? "", { level: "project" }), enabled: !!project && can("limits.view") });
  const showErrors = !!project && logsOn && can("logs.metadata.view");
  const errors = useQuery({ ...recentErrorsQuery(org, project ?? "", 4), enabled: showErrors });
  const providers = useProviders(org);
  const partyName = usePartyName(org, project ?? "");

  const modelName = (key: string) => {
    const id = key.split("/").slice(1).join("/");
    if (!id) return t("project.overview.noModel", "No model");
    return models.data?.find((m) => m.model_id === id && key.startsWith(`${m.provider}/`))?.effective.display_name || id;
  };
  const keyName = (id: string) => {
    if (id === "playground") return t("project.overview.playground", "Playground");
    const key = keys.data?.find((k) => k.id === id);
    return key ? keyLabel(key) : id;
  };
  const memberName = (k: string) => {
    const [kind, id = ""] = k.split(":");
    const sa = kind === "service_account";
    if (!project) {
      if (sa) return t("orgHome.serviceAccount", "Service account");
      const m = orgMembers.data?.find((x) => x.user.id === id);
      if (!m) return t("orgHome.formerMember", "Former member");
      return m.user.id === session.user.id ? t("orgHome.nameYou", "{name} (you)", { name: m.user.name }) : m.user.name;
    }
    const name = partyName({ kind: sa ? "service_account" : "user", id });
    return sa ? serviceAccountName(name) : name;
  };
  const projectOf = (id: string) => projects.data?.find((p) => p.id === id);

  const rows = daily.data ?? [];
  const prevDaily = previous.data;
  const points = fineStep
    ? fill(fine.data, startMs, endMs, fineStep)
    : byBucket(daily.data ?? [], from, to, g).map((d) => ({ t: new Date(`${d.day}T00:00:00Z`), row: d.row }));
  const prevPoints = fineStep
    ? fill(finePrev.data, startMs - periodMs, endMs - periodMs, fineStep)
    : byBucket(prevDaily ?? [], before.from, before.to, g).map((d) => ({ t: new Date(`${d.day}T00:00:00Z`), row: d.row }));
  const prevRowsByIndex = byBucket(previous.data ?? [], before.from, before.to, g);
  const prevRows = prevRowsByIndex.flatMap((x) => (x.row ? [x.row] : []));

  return {
    org,
    project,
    from,
    to,
    period,
    g,
    compare,
    compareLabel: compare ?? "",
    canUsage,
    showCost,
    can,
    daily,
    rows,
    days: byBucket(rows, from, to, g),
    prevDays: prevRowsByIndex,
    prevRows: compare && previous.data && prevRows.some((r) => r.requests > 0) ? prevRows : undefined,
    now: totals(rows),
    loading: daily.isPending || (!!compare && previous.isLoading),
    refreshing: [daily, previous, byFirst, bySecond, byThird].some((q) => q.isPlaceholderData),
    single,
    step: fineStep,
    points,
    prevPoints,
    pointsLoading: fineStep ? fine.isPending : daily.isPending,
    context: single ? { query: context, days: byBucket(context.data ?? [], contextFrom, to, "day") } : undefined,
    grouped,
    spend: limits.data?.find((l) => l.kind === "spend_budget"),
    limits,
    errors,
    showErrors,
    providers,
    projects,
    modelName,
    keyName,
    memberName,
    projectOf,
    namesLoading: models.isLoading || keys.isLoading || projects.isLoading || orgMembers.isLoading,
    live: to === utcDay(new Date()),
    forecast: showCost && g === "day" && to === utcDay(new Date()) ? forecast(rows, from, to) : undefined,
  };
}

export type Overview = ReturnType<typeof useOverview>;
