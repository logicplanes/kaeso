import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { OrgRetirements } from "../generated/types";

export const orgRetirementsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "deprecations"],
    queryFn: async (): Promise<OrgRetirements> => data(await api.GET("/platform/v1/orgs/{org}/deprecations", { params: { path: { org } } })),
    staleTime: 300_000,
  });
