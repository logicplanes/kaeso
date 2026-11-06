import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { PortalGrantView, ProjectMcpServerView, ProjectToolView, ToolAccess } from "../generated/types";

const base = (org: string, project: string) => ["orgs", org, "projects", project, "tools"] as const;

export const projectToolsQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: [...base(org, project), "list"],
    queryFn: async (): Promise<ProjectToolView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/tools", { params: { path: { org, project } } })).data,
  });

export const projectServersQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: [...base(org, project), "servers"],
    queryFn: async (): Promise<ProjectMcpServerView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/mcp-servers", { params: { path: { org, project } } })).data,
  });

export const portalGrantsQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: [...base(org, project), "grants"],
    queryFn: async (): Promise<PortalGrantView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/portal-grants", { params: { path: { org, project } } })).data,
  });

export const toolAccessQuery = (org: string, project: string, principal: string) =>
  queryOptions({
    queryKey: [...base(org, project), "access", principal],
    queryFn: async (): Promise<ToolAccess> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/tool-access/{principal}", { params: { path: { org, project, principal } } })),
  });
