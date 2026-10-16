import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { PlaygroundModel } from "../generated/types";

export type { PlaygroundModel };

export const playgroundModelsQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: ["orgs", org, "projects", project, "playground", "models"],
    queryFn: async (): Promise<PlaygroundModel[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/playground/models", { params: { path: { org, project } } })).data,
    staleTime: 60_000,
  });
