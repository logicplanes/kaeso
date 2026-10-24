import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { CustomProviderView, OfferedModel } from "../generated/types";

export const customProvidersQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "custom-providers"],
    queryFn: async (): Promise<CustomProviderView[]> => data(await api.GET("/platform/v1/orgs/{org}/custom-providers", { params: { path: { org } } })).data,
    staleTime: 60_000,
  });

export const offeredModelsQuery = (org: string, project: string, provider: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "models", "offered", provider],
    queryFn: async ({ signal }): Promise<OfferedModel[]> =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/models/offered", {
          params: { path: { org, project }, query: { provider } },
          signal,
        }),
      ).data,
    staleTime: 60_000,
    retry: false,
  });
