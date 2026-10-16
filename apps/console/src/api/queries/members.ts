import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ModelAccess, ProjectMemberView, ServiceAccountView } from "../generated/types";
import { allPages } from "../paging";

export const projectMembersQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "members"],
    queryFn: (): Promise<ProjectMemberView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/members", {
            params: { path: { org, project }, query: { limit: 100, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });

export const serviceAccountsQuery = (org: string, project: string, includeArchived = false) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "service-accounts", { includeArchived }],
    queryFn: (): Promise<ServiceAccountView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/service-accounts", {
            params: { path: { org, project }, query: { limit: 100, include_archived: includeArchived, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });

export type Principal = { kind: "user"; id: string } | { kind: "service_account"; id: string };

export const modelAccessQuery = (org: string, project: string, subject: Principal | { kind: "key"; id: string }) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "model-access", subject],
    queryFn: async (): Promise<ModelAccess> => {
      const path = { org, project };
      if (subject.kind === "user")
        return data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/members/{user}/models", { params: { path: { ...path, user: subject.id } } }));
      if (subject.kind === "service_account")
        return data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}/models", { params: { path: { ...path, account: subject.id } } }),
        );
      return data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/keys/{key}/models", { params: { path: { ...path, key: subject.id } } }));
    },
  });
