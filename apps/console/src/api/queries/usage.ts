import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { UsageRow, UsageSpan } from "../generated/types";

export type GroupBy = "day" | "week" | "month" | "model" | "key" | "member" | "project";

const CHARTED = { refetchOnWindowFocus: false } as const;

export const projectUsageQuery = (org: string, project: string, from: string, to: string, groupBy: GroupBy) =>
  queryOptions({
    ...CHARTED,
    queryKey: ["orgs", org, "projects", project, "usage", { from, to, groupBy }],
    queryFn: async (): Promise<UsageRow[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/usage", { params: { path: { org, project }, query: { from, to, group_by: groupBy } } }))
        .data,
  });

export const orgUsageQuery = (org: string, from: string, to: string, groupBy: GroupBy) =>
  queryOptions({
    ...CHARTED,
    queryKey: ["orgs", org, "usage", { from, to, groupBy }],
    queryFn: async (): Promise<UsageRow[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/usage", { params: { path: { org }, query: { from, to, group_by: groupBy } } })).data,
  });

export const usageSpanQuery = (org: string, project?: string) =>
  queryOptions({
    queryKey: project ? ["orgs", org, "projects", project, "usage", "span"] : ["orgs", org, "usage", "span"],
    staleTime: 60_000,
    queryFn: async (): Promise<UsageSpan> =>
      project
        ? data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/usage/span", { params: { path: { org, project } } }))
        : data(await api.GET("/platform/v1/orgs/{org}/usage/span", { params: { path: { org } } })),
  });
