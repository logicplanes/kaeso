import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { Me } from "../generated/types";

export const meQuery = queryOptions({
  queryKey: ["me"],
  queryFn: async (): Promise<Me> => data(await api.GET("/platform/v1/me")),
  staleTime: 60_000,
});
