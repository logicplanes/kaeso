import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ModelLifecycleSettings, ProjectLogSettings, ProjectResponseCache, ProjectStoredResponses } from "../generated/types";

export const projectLogSettingsQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "logs", "settings"] as const,
    queryFn: async (): Promise<ProjectLogSettings> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/settings", { params: { path: { org, project } } })),
  });

export const projectResponseCacheQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "response-cache"] as const,
    queryFn: async (): Promise<ProjectResponseCache> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/response-cache", { params: { path: { org, project } } })),
  });

export const projectStoredResponsesQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "stored-responses"] as const,
    queryFn: async (): Promise<ProjectStoredResponses> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/stored-responses", { params: { path: { org, project } } })),
  });

export const modelLifecycleQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "model-lifecycle"] as const,
    queryFn: async (): Promise<ModelLifecycleSettings> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/model-lifecycle", { params: { path: { org, project } } })),
  });
