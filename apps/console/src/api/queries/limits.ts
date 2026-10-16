import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { LimitView } from "../generated/types";

export interface LimitFilter {
  level?: "project" | "member" | "key";
  user?: string;
  service_account?: string;
  key?: string;
}

export const limitsQuery = (org: string, project: string, filter: LimitFilter = {}) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "limits", filter],
    queryFn: async (): Promise<LimitView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/limits", { params: { path: { org, project }, query: filter } })).data,
  });
