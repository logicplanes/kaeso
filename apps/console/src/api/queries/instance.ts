import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type {
  InstanceOrgUsage,
  InstanceOrgView,
  InstanceUserView,
  MemberView,
  ModelsDevStatus,
  ProjectView,
  SettingsView,
  SystemView,
} from "../generated/types";
import { allPages, nextPageParam } from "../paging";

export const instanceUsersQuery = (search: string, state: string | undefined) =>
  infiniteQueryOptions({
    queryKey: ["instance", "users", { search, state }],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam, signal }) =>
      data(
        await api.GET("/platform/v1/instance/users", {
          params: { query: { limit: 100, ...(search ? { search } : {}), ...(state ? { state } : {}), ...(pageParam ? { after: pageParam } : {}) } },
          signal,
        }),
      ),
    getNextPageParam: nextPageParam,
  });

export const instanceUserQuery = (user: string) =>
  queryOptions({
    queryKey: ["instance", "users", "one", user],
    queryFn: async (): Promise<InstanceUserView> => data(await api.GET("/platform/v1/instance/users/{user}", { params: { path: { user } } })),
  });

export const instanceOrgsQuery = (search: string, sort: "created" | "deletion" = "created") =>
  infiniteQueryOptions({
    queryKey: ["instance", "orgs", { search, sort }],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam, signal }) =>
      data(
        await api.GET("/platform/v1/instance/orgs", {
          params: {
            query: { limit: 100, ...(search ? { search } : {}), ...(sort === "deletion" ? { sort } : {}), ...(pageParam ? { after: pageParam } : {}) },
          },
          signal,
        }),
      ),
    getNextPageParam: nextPageParam,
  });

export const instanceOrgQuery = (org: string) =>
  queryOptions({
    queryKey: ["instance", "orgs", "one", org],
    queryFn: async (): Promise<InstanceOrgView> => data(await api.GET("/platform/v1/instance/orgs/{org}", { params: { path: { org } } })),
  });

export const instanceOrgUsageQuery = (org: string) =>
  queryOptions({
    queryKey: ["instance", "orgs", "one", org, "usage"],
    queryFn: async (): Promise<InstanceOrgUsage> => data(await api.GET("/platform/v1/instance/orgs/{org}/usage", { params: { path: { org } } })),
  });

export const instanceOrgMembersQuery = (org: string) =>
  queryOptions({
    queryKey: ["instance", "orgs", "one", org, "members"],
    queryFn: (): Promise<MemberView[]> =>
      allPages(async (after) =>
        data(await api.GET("/platform/v1/instance/orgs/{org}/members", { params: { path: { org }, query: { limit: 100, ...(after ? { after } : {}) } } })),
      ),
  });

export const instanceOrgProjectsQuery = (org: string) =>
  queryOptions({
    queryKey: ["instance", "orgs", "one", org, "projects"],
    queryFn: (): Promise<ProjectView[]> =>
      allPages(async (after) =>
        data(await api.GET("/platform/v1/instance/orgs/{org}/projects", { params: { path: { org }, query: { limit: 100, ...(after ? { after } : {}) } } })),
      ),
  });

export const instanceSettingsQuery = queryOptions({
  queryKey: ["instance", "settings"],
  queryFn: async (): Promise<SettingsView> => data(await api.GET("/platform/v1/instance/settings")),
});

export const modelsDevQuery = queryOptions({
  queryKey: ["instance", "models-dev"],
  queryFn: async (): Promise<ModelsDevStatus> => data(await api.GET("/platform/v1/instance/models-dev")),
});

export const systemQuery = queryOptions({
  queryKey: ["instance", "system"],
  queryFn: async (): Promise<SystemView> => data(await api.GET("/platform/v1/instance/system")),
});
