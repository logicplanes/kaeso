import type { GroupBy } from "~/api/queries/usage";
import type { OrgBreakdown } from "~/features/org-home/org-usage-breakdown";
import { periodSearch, type PeriodSearch } from "~/format/period-search";
import type { MetricId } from "./metrics";

export const PROJECT_DIMENSIONS = ["models", "keys", "members"] as const;
export const PROJECT_VIEWS = ["reliability", "efficiency", ...PROJECT_DIMENSIONS, "breakdown"] as const;
export type ProjectView = (typeof PROJECT_VIEWS)[number];
export const ORG_DIMENSIONS = ["projects", "models", "members"] as const;
export const ORG_VIEWS = ["reliability", "efficiency", ...ORG_DIMENSIONS, "breakdown"] as const;
export type OrgView = (typeof ORG_VIEWS)[number];

export type AnalyticsSearch = PeriodSearch & { by?: Exclude<GroupBy, "project" | "week" | "month" | "billed_to"> | "client"; metric?: MetricId };
export type OrgAnalyticsSearch = PeriodSearch & { by?: OrgBreakdown; metric?: MetricId };

const PROJECT_BY = ["day", "model", "key", "member", "route", "client", "source"] as const;
const ORG_BY = ["project", "model", "member", "billed_to"] as const;
const SEARCHED = ["requests", "tokens", "cost", "errors", "tools"] as const;

export function analyticsSearch(s: Record<string, unknown>): AnalyticsSearch {
  const by = PROJECT_BY.find((b) => b === s.by);
  const metric = SEARCHED.find((m) => m === s.metric);
  return { ...periodSearch(s), ...(by ? { by } : {}), ...(metric ? { metric } : {}) };
}

export function orgAnalyticsSearch(s: Record<string, unknown>): OrgAnalyticsSearch {
  const by = ORG_BY.find((b) => b === s.by);
  const metric = SEARCHED.find((m) => m === s.metric);
  return { ...periodSearch(s), ...(by ? { by } : {}), ...(metric ? { metric } : {}) };
}
