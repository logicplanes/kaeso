import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { KeyView } from "../generated/types";
import { allPages } from "../paging";

export interface KeyFilter {
  user?: string;
  service_account?: string;
  include_archived?: boolean;
}

export function keyFilter({ owner, archived }: { owner?: string | undefined; archived?: boolean | undefined }): KeyFilter {
  return {
    ...(owner?.startsWith("user:") ? { user: owner.slice(5) } : {}),
    ...(owner?.startsWith("sa:") ? { service_account: owner.slice(3) } : {}),
    ...(archived ? { include_archived: true } : {}),
  };
}

export const keysPageQuery = (org: string, project: string, filter: KeyFilter = {}) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "keys", filter],
    queryFn: async () =>
      data(
        await api.GET("/platform/v1/orgs/{org}/projects/{project}/keys", {
          params: { path: { org, project }, query: { ...filter, limit: 100 } },
        }),
      ),
  });

const unarchivedKeys = (keys: KeyView[]): KeyView[] => keys.filter((k) => k.state !== "archived");

export const allKeysQuery = (org: string, project: string, { include_archived, ...owner }: KeyFilter = {}) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "keys", "all", owner],
    queryFn: (): Promise<KeyView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/keys", {
            params: { path: { org, project }, query: { ...owner, include_archived: true, limit: 100, ...(after ? { after } : {}) } },
          }),
        ),
      ),
    ...(include_archived ? {} : { select: unarchivedKeys }),
  });
