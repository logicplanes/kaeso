import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { KeyView } from "../generated/types";
import { allPages } from "../paging";

export interface KeyFilter {
  user?: string;
  service_account?: string;
  include_archived?: boolean;
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

export const allKeysQuery = (org: string, project: string, filter: KeyFilter = {}) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "keys", "all", filter],
    queryFn: (): Promise<KeyView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/keys", {
            params: { path: { org, project }, query: { ...filter, limit: 100, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });
