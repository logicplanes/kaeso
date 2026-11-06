import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { PersonConnectionView } from "../generated/types";

export const myConnectionsQuery = queryOptions({
  queryKey: ["me", "mcp-connections"],
  queryFn: async (): Promise<PersonConnectionView[]> => data(await api.GET("/platform/v1/me/mcp-connections")).data,
  refetchOnWindowFocus: true,
});
