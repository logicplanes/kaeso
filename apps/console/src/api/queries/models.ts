import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import { isApiError } from "../errors";
import type { AliasView, CatalogueModelView, ModelSearch, Preview } from "../generated/types";

export const catalogueQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models"],
    queryFn: async (): Promise<CatalogueModelView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/models", { params: { path: { org, project } } })).data,
    staleTime: 60_000,
  });

export const aliasesQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "aliases"],
    queryFn: async (): Promise<AliasView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/aliases", { params: { path: { org, project } } })).data,
    staleTime: 60_000,
  });

export const catalogueModelQuery = (org: string, project: string, model: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models", "one", model],
    queryFn: async (): Promise<CatalogueModelView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/models/{model}", { params: { path: { org, project, model } } })),
  });

export const SEARCH_LIMIT = 50;

export const modelSearchQuery = (org: string, project: string, q: { q: string; provider: string; include_deprecated: boolean }) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models", "search", q],
    queryFn: async ({ signal }): Promise<ModelSearch> =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/models/search", {
          params: {
            path: { org, project },
            query: { ...(q.q ? { q: q.q } : {}), provider: q.provider, include_deprecated: q.include_deprecated, limit: SEARCH_LIMIT },
          },
          signal,
        }),
      ),
    staleTime: 60_000,
  });

export const currentModelsQuery = (org: string, project: string, provider: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models", "search", { provider, current: true }],
    queryFn: async ({ signal }): Promise<ModelSearch> =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/models/search", {
          params: { path: { org, project }, query: { provider, current: true, limit: 1 } },
          signal,
        }),
      ),
    staleTime: 60_000,
  });

export const modelPreviewQuery = (org: string, project: string, provider: string, model: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models", "preview", provider, model],
    queryFn: async (): Promise<Preview | null> => {
      try {
        return data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/models/preview", { params: { path: { org, project }, query: { provider, model } } }),
        );
      } catch (e) {
        if (isApiError(e) && e.type === "not_found") return null;
        throw e;
      }
    },
    staleTime: 60_000,
  });
