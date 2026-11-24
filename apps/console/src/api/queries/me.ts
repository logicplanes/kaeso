import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { KeyView, Me, NotificationSettings, SessionListItem, SsoLinks } from "../generated/types";
import { allPages } from "../paging";

export const meQuery = queryOptions({
  queryKey: ["me"],
  queryFn: async (): Promise<Me> => data(await api.GET("/platform/v1/me")),
  staleTime: 60_000,
});

export const ssoLinksQuery = queryOptions({
  queryKey: ["me", "sso-links"],
  queryFn: async (): Promise<SsoLinks> => data(await api.GET("/platform/v1/me/sso-links")),
});

export const mySessionsQuery = queryOptions({
  queryKey: ["me", "sessions"],
  queryFn: async (): Promise<SessionListItem[]> => data(await api.GET("/platform/v1/me/sessions")).data,
});

export const myKeysQuery = (includeArchived: boolean) =>
  queryOptions({
    queryKey: ["me", "keys", { includeArchived }],
    queryFn: (): Promise<KeyView[]> =>
      allPages(async (after) =>
        data(await api.GET("/platform/v1/me/keys", { params: { query: { limit: 100, include_archived: includeArchived, ...(after ? { after } : {}) } } })),
      ),
  });

export const notificationsQuery = queryOptions({
  queryKey: ["me", "notifications"],
  queryFn: async (): Promise<NotificationSettings> => data(await api.GET("/platform/v1/me/notifications")),
});
