import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { SharedLinkView, SharedView } from "../generated/types";

export const sharedLinksKey = (org: string) => ["orgs", org, "shared-links"];

export const sharedLinksQuery = (org: string) =>
  queryOptions({
    queryKey: sharedLinksKey(org),
    queryFn: async (): Promise<SharedLinkView[]> => data(await api.GET("/platform/v1/orgs/{org}/shared-links", { params: { path: { org } } })).data,
  });

export const sharedViewQuery = (token: string) =>
  queryOptions({
    queryKey: ["share", token],
    retry: false,
    refetchOnWindowFocus: false,
    meta: { quiet: true },
    queryFn: async (): Promise<SharedView> => data(await api.GET("/platform/v1/share/{token}", { params: { path: { token } } })),
  });
