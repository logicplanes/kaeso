import { queryOptions } from "@tanstack/react-query";
import { api, data } from "~/api/client";
import type { MyMembership, WhoAmI } from "~/api/generated/types";

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: async (): Promise<WhoAmI> => {
    const [who, { setFormatTimeZone }] = await Promise.all([api.GET("/platform/v1/auth/session").then(data), import("@logicplanes/kaeso-ui/format")]);
    setFormatTimeZone(who.timezone);
    return who;
  },
  staleTime: 5_000,
});

export const membershipsQuery = queryOptions({
  queryKey: ["me", "memberships"],
  queryFn: async (): Promise<MyMembership[]> => data(await api.GET("/platform/v1/me/memberships")).data,
});
