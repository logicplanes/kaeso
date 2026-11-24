import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { OrgRetirements, ProjectView } from "../generated/types";
import { allPages, unarchived } from "../paging";

export const projectListKey = (org: string) => ["orgs", org, "project-list"] as const;

export const projectViewKey = (org: string, project: string) => ["orgs", org, "projects", project, "view"] as const;

export const projectQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: projectViewKey(org, project),
    queryFn: async (): Promise<ProjectView> => data(await api.GET("/platform/v1/orgs/{org}/projects/{project}", { params: { path: { org, project } } })),
    staleTime: 60_000,
    retryOnMount: false,
  });

export const orgProjectsQuery = (org: string, includeArchived = false) =>
  queryOptions({
    queryKey: projectListKey(org),
    queryFn: (): Promise<ProjectView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects", {
            params: { path: { org }, query: { limit: 100, include_archived: true, ...(after ? { after } : {}) } },
          }),
        ),
      ),
    ...(includeArchived ? {} : { select: unarchived<ProjectView> }),
  });

export const orgRetirementsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "deprecations"],
    queryFn: async (): Promise<OrgRetirements> => data(await api.GET("/platform/v1/orgs/{org}/deprecations", { params: { path: { org } } })),
    staleTime: 300_000,
  });
