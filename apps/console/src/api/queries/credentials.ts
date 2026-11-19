import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { CredentialView, LinkedCosts, LinkedFallback } from "../generated/types";

export const orgCredentialsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "credentials"],
    queryFn: async (): Promise<CredentialView[]> => data(await api.GET("/platform/v1/orgs/{org}/credentials", { params: { path: { org } } })).data,
    staleTime: 60_000,
  });

export const linkedFallbacksQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "credentials", "fallbacks"],
    queryFn: async (): Promise<LinkedFallback[]> => data(await api.GET("/platform/v1/orgs/{org}/linked-billing/fallbacks", { params: { path: { org } } })),
    staleTime: 60_000,
  });

export const linkedCostsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "credentials", "linked-costs"],
    queryFn: async (): Promise<LinkedCosts> => data(await api.GET("/platform/v1/orgs/{org}/linked-billing/costs", { params: { path: { org } } })),
    staleTime: 60_000,
  });
