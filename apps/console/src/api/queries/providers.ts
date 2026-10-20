import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ProviderView } from "../generated/types";

export const providersQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "providers"],
    queryFn: async (): Promise<ProviderView[]> => data(await api.GET("/platform/v1/orgs/{org}/providers", { params: { path: { org } } })).data,
    staleTime: 60 * 60_000,
    meta: { quiet: true },
  });
