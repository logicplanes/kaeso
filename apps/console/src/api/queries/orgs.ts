import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { InviteView, LogJobView, MemberView, MemberWithout2fa, OrgKeyView, OrgLogSettings, OrgView, SecurityView, StorageView } from "../generated/types";
import { allPages } from "../paging";

export const orgViewKey = (org: string) => ["orgs", org, "view"] as const;

export const orgQuery = (org: string) =>
  queryOptions({
    queryKey: orgViewKey(org),
    queryFn: async (): Promise<OrgView> => data(await api.GET("/platform/v1/orgs/{org}", { params: { path: { org } } })),
    staleTime: 60_000,
  });

export const orgMembersQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "members"],
    queryFn: (): Promise<MemberView[]> =>
      allPages(async (after) =>
        data(await api.GET("/platform/v1/orgs/{org}/members", { params: { path: { org }, query: { limit: 100, ...(after ? { after } : {}) } } })),
      ),
  });

// One person of the organization, for naming someone a project shows without them being its member.
export const orgMemberQuery = (org: string, user: string) =>
  queryOptions({
    queryKey: ["orgs", org, "member", user],
    queryFn: async (): Promise<MemberView> => data(await api.GET("/platform/v1/orgs/{org}/members/{user}", { params: { path: { org, user } } })),
    retry: false,
    staleTime: 5 * 60_000,
    meta: { quiet: true },
  });

export const orgOwnersQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "members", { role: "owner" }],
    queryFn: async (): Promise<MemberView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/members", { params: { path: { org }, query: { role: "owner", limit: 3 } } })).data,
  });

export const invitesQuery = (org: string, pendingOnly: boolean) =>
  queryOptions({
    queryKey: ["orgs", org, "invites", { pendingOnly }],
    queryFn: (): Promise<InviteView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/invites", {
            params: { path: { org }, query: { limit: 100, pending: pendingOnly, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });

export const platformKeyQuery = (org: string, key: string) =>
  queryOptions({
    queryKey: ["orgs", org, "platform-keys", "one", key],
    queryFn: async (): Promise<OrgKeyView> => data(await api.GET("/platform/v1/orgs/{org}/platform-keys/{key}", { params: { path: { org, key } } })),
  });

export const platformKeysQuery = (org: string, includeArchived: boolean) =>
  queryOptions({
    queryKey: ["orgs", org, "platform-keys", { includeArchived }],
    queryFn: (): Promise<OrgKeyView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/platform-keys", {
            params: { path: { org }, query: { limit: 100, include_archived: includeArchived, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });

export const securityQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "security"],
    queryFn: async (): Promise<SecurityView> => data(await api.GET("/platform/v1/orgs/{org}/security", { params: { path: { org } } })),
  });

export const membersWithout2faQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "security", "members-without-2fa"],
    queryFn: async (): Promise<MemberWithout2fa[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/security/members-without-2fa", { params: { path: { org } } })).data,
  });

export const logSettingsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "logs", "settings"],
    queryFn: async (): Promise<OrgLogSettings> => data(await api.GET("/platform/v1/orgs/{org}/logs/settings", { params: { path: { org } } })),
  });

export const logStorageQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "logs", "storage"],
    queryFn: async (): Promise<StorageView> => data(await api.GET("/platform/v1/orgs/{org}/logs/storage", { params: { path: { org } } })),
  });

export const logDeletionsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "logs", "deletions"],
    queryFn: async (): Promise<LogJobView[]> => data(await api.GET("/platform/v1/orgs/{org}/logs/deletions", { params: { path: { org } } })).data,
  });
