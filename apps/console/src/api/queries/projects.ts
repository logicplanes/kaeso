import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ProjectView } from "../generated/types";
import { allPages } from "../paging";

export const projectQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project],
    queryFn: async (): Promise<ProjectView> => data(await api.GET("/platform/v1/orgs/{org}/projects/{project}", { params: { path: { org, project } } })),
    staleTime: 60_000,
    retryOnMount: false,
  });

export const orgProjectsQuery = (org: string, includeArchived = false) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", { includeArchived }],
    queryFn: (): Promise<ProjectView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects", {
            params: { path: { org }, query: { limit: 100, include_archived: includeArchived, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });
