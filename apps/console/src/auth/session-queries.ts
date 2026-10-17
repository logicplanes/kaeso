import { queryOptions } from "@tanstack/react-query";
import { api, data } from "~/api/client";
import type { MyMembership, WhoAmI } from "~/api/generated/types";

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: async (): Promise<WhoAmI> => data(await api.GET("/platform/v1/auth/session")),
  staleTime: 0,
});

export const membershipsQuery = queryOptions({
  queryKey: ["me", "memberships"],
  queryFn: async (): Promise<MyMembership[]> => data(await api.GET("/platform/v1/me/memberships")).data,
});
