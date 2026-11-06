import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { McpServerView, McpToolChange, McpToolView } from "../generated/types";

export const mcpServersQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "mcp-servers"],
    queryFn: async (): Promise<McpServerView[]> => data(await api.GET("/platform/v1/orgs/{org}/mcp-servers", { params: { path: { org } } })).data,
    staleTime: 30_000,
  });

export const mcpServerQuery = (org: string, server: string) =>
  queryOptions({
    queryKey: ["orgs", org, "mcp-servers", server],
    queryFn: async (): Promise<McpServerView> => data(await api.GET("/platform/v1/orgs/{org}/mcp-servers/{server}", { params: { path: { org, server } } })),
  });

export const mcpServerToolsQuery = (org: string, server: string) =>
  queryOptions({
    queryKey: ["orgs", org, "mcp-servers", server, "tools"],
    queryFn: async (): Promise<McpToolView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/mcp-servers/{server}/tools", { params: { path: { org, server } } })).data,
  });

export const mcpToolChangeQuery = (org: string, server: string, tool: string) =>
  queryOptions({
    queryKey: ["orgs", org, "mcp-servers", server, "tools", tool, "change"],
    queryFn: async (): Promise<McpToolChange> =>
      data(await api.GET("/platform/v1/orgs/{org}/mcp-servers/{server}/tools/{tool}/change", { params: { path: { org, server, tool } } })),
  });
