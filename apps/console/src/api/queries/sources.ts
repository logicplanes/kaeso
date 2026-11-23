import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ExternalSourceView } from "../generated/types";
import { allPages } from "../paging";

/** A project's sources, archived ones included, so a past request still names its source. */
export const sourcesQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "sources", "list"],
    queryFn: (): Promise<ExternalSourceView[]> =>
      allPages(async (after) =>
        data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/sources", {
            params: { path: { org, project }, query: { limit: 100, include_archived: true, ...(after ? { after } : {}) } },
          }),
        ),
      ),
  });

export const sourceQuery = (org: string, project: string, source: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "sources", "one", source],
    queryFn: async (): Promise<ExternalSourceView> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/sources/{source}", { params: { path: { org, project, source } } })),
  });
