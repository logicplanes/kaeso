import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { WhatsNewView } from "../generated/types";

export const whatsNewQuery = queryOptions({
  queryKey: ["me", "whats-new"],
  queryFn: async (): Promise<WhatsNewView> => data(await api.GET("/platform/v1/me/whats-new")),
  staleTime: Infinity,
});
