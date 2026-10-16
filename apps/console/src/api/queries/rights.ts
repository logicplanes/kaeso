import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { MemberRights, MemberUser, ProjectRightsView, RightsCatalogue, RoleView } from "../generated/types";

export const rightsCatalogueQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "rights-catalogue"],
    queryFn: async (): Promise<RightsCatalogue> => data(await api.GET("/platform/v1/orgs/{org}/rights", { params: { path: { org } } })),
    staleTime: 30 * 60_000,
  });

export const rolesQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "roles"],
    queryFn: async (): Promise<RoleView[]> => data(await api.GET("/platform/v1/orgs/{org}/roles", { params: { path: { org } } })).data,
    staleTime: 60_000,
  });

export const memberRightsQuery = (org: string, user: string) =>
  queryOptions({
    queryKey: ["orgs", org, "members", "rights", user],
    queryFn: async (): Promise<MemberRights> => data(await api.GET("/platform/v1/orgs/{org}/members/{user}/rights", { params: { path: { org, user } } })),
  });

export const projectMemberRightsQuery = (org: string, project: string, user: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "members", "rights", user],
    queryFn: async (): Promise<ProjectRightsView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/members/{user}/rights", { params: { path: { org, project, user } } })),
  });

export const grantorsQuery = (org: string, right: string) =>
  queryOptions({
    queryKey: ["orgs", org, "grantors", right],
    queryFn: async (): Promise<MemberUser[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/rights/{right}/grantors", { params: { path: { org, right } } })).data,
    staleTime: 5 * 60_000,
    retry: false,
    meta: { quiet: true },
  });

export const projectGrantorsQuery = (org: string, project: string, right: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "grantors", right],
    queryFn: async (): Promise<MemberUser[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/rights/{right}/grantors", { params: { path: { org, project, right } } })).data,
    staleTime: 5 * 60_000,
    retry: false,
    meta: { quiet: true },
  });
