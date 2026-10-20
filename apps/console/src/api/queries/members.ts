import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ModelAccess, ProjectMemberView, ServiceAccountView } from "../generated/types";
import { allPages, unarchived } from "../paging";

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
    queryKey: ["orgs", org, "projects", project, "service-accounts", "list"],
    queryFn: (): Promise<ServiceAccountView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/service-accounts", {
            params: { path: { org, project }, query: { limit: 100, include_archived: true, ...(after ? { after } : {}) } },
          }),
        ),
      ),
    ...(includeArchived ? {} : { select: unarchived<ServiceAccountView> }),
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

const LOOKUP_BATCH = 4;

export interface FoundServiceAccount {
  account: ServiceAccountView;
  project: string;
}

export interface LookupProject {
  id: string;
  name: string;
}

export const serviceAccountLookupQuery = (org: string, projects: readonly LookupProject[], ids: readonly string[]) =>
  queryOptions({
    queryKey: ["orgs", org, "service-account-lookup", { projects, ids }],
    queryFn: async ({ client }): Promise<FoundServiceAccount[]> => {
      const wanted = new Set(ids);
      const found: FoundServiceAccount[] = [];
      for (let i = 0; i < projects.length && found.length < wanted.size; i += LOOKUP_BATCH) {
        const batch = projects.slice(i, i + LOOKUP_BATCH);
        const lists = await Promise.allSettled(
          batch.map(async (project) => ({ project: project.name, accounts: await client.fetchQuery(serviceAccountsQuery(org, project.id, true)) })),
        );
        for (const list of lists) {
          if (list.status === "rejected") continue;
          for (const account of list.value.accounts) if (wanted.has(account.id)) found.push({ account, project: list.value.project });
        }
      }
      return found;
    },
  });
