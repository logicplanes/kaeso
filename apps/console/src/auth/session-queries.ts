import { queryOptions } from "@tanstack/react-query";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { MyMembership, WhoAmI } from "~/api/generated/types";

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: async (): Promise<WhoAmI> => {
    const who = data(await api.GET("/platform/v1/auth/session"));
    setFormatTimeZone(who.timezone);
    return who;
  },
  staleTime: 5_000,
});

export const membershipsQuery = queryOptions({
  queryKey: ["me", "memberships"],
  queryFn: async (): Promise<MyMembership[]> => data(await api.GET("/platform/v1/me/memberships")).data,
});
