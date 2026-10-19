import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { Me, SsoLinks } from "../generated/types";

export const meQuery = queryOptions({
  queryKey: ["me"],
  queryFn: async (): Promise<Me> => data(await api.GET("/platform/v1/me")),
  staleTime: 60_000,
});

export const ssoLinksQuery = queryOptions({
  queryKey: ["me", "sso-links"],
  queryFn: async (): Promise<SsoLinks> => data(await api.GET("/platform/v1/me/sso-links")),
});
