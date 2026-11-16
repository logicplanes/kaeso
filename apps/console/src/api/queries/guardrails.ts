import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { GuardrailView, ProjectGuardrailView } from "../generated/types";

export const guardrailsKey = (org: string) => ["orgs", org, "guardrails"] as const;

export const guardrailsQuery = (org: string) =>
  queryOptions({
    queryKey: guardrailsKey(org),
    queryFn: async (): Promise<GuardrailView[]> => data(await api.GET("/platform/v1/orgs/{org}/guardrails", { params: { path: { org } } })).data,
    staleTime: 30_000,
  });

export const projectGuardrailsKey = (org: string, project: string) => ["orgs", org, "projects", project, "guardrails"] as const;

export const projectGuardrailsQuery = (org: string, project: string) =>
  queryOptions({
    queryKey: projectGuardrailsKey(org, project),
    queryFn: async (): Promise<ProjectGuardrailView[]> =>
      data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/guardrails", { params: { path: { org, project } } })).data,
  });
