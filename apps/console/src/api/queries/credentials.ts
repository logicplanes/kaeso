import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { CredentialView } from "../generated/types";

export const orgCredentialsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "credentials"],
    queryFn: async (): Promise<CredentialView[]> => data(await api.GET("/platform/v1/orgs/{org}/credentials", { params: { path: { org } } })).data,
    staleTime: 60_000,
  });
