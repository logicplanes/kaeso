import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { LogView } from "../generated/types";

export const recentErrorsQuery = (org: string, project: string, limit = 5) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", { failed: true, limit }],
    queryFn: async (): Promise<LogView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs", { params: { path: { org, project }, query: { failed: true, limit } } })).data,
  });
