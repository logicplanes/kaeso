import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import { nextPageParam } from "../paging";
import type { AgentCatalogue, AgentView, Grantable, InstanceAgentPolicy, List_AgentActivityView, OrgAgentPolicy, OrgAgentView } from "../generated/types";

export const agentsQuery = queryOptions({
  queryKey: ["me", "agents"],
  queryFn: async (): Promise<AgentView[]> => data(await api.GET("/platform/v1/me/agents")).data,
});

export const agentQuery = (agent: string) =>
  queryOptions({
    queryKey: ["me", "agents", agent],
    queryFn: async (): Promise<AgentView> => data(await api.GET("/platform/v1/me/agents/{agent}", { params: { path: { agent } } })),
  });

export const agentCatalogueQuery = queryOptions({
  queryKey: ["agent-catalogue"],
  queryFn: async (): Promise<AgentCatalogue> => data(await api.GET("/platform/v1/me/agents/catalogue")),
  staleTime: Infinity,
});

export const grantableQuery = queryOptions({
  queryKey: ["me", "agents", "grantable"],
  queryFn: async (): Promise<Grantable> => data(await api.GET("/platform/v1/me/agents/grantable")),
});

export const agentActivityQuery = (agent: string) =>
  infiniteQueryOptions({
    queryKey: ["me", "agents", agent, "activity"],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam }): Promise<List_AgentActivityView> =>
      data(
        await api.GET("/platform/v1/me/agents/{agent}/activity", {
          params: { path: { agent }, query: { limit: 50, ...(pageParam ? { after: pageParam } : {}) } },
        }),
      ),
    getNextPageParam: nextPageParam,
  });

export const orgAgentsQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "agents"],
    queryFn: async (): Promise<OrgAgentView[]> => data(await api.GET("/platform/v1/orgs/{org}/agents", { params: { path: { org } } })).data,
  });

export const orgAgentPolicyQuery = (org: string) =>
  queryOptions({
    queryKey: ["orgs", org, "agent-policy"],
    queryFn: async (): Promise<OrgAgentPolicy> => data(await api.GET("/platform/v1/orgs/{org}/security/agents", { params: { path: { org } } })),
  });

export const instanceAgentPolicyQuery = queryOptions({
  queryKey: ["instance", "agent-policy"],
  queryFn: async (): Promise<InstanceAgentPolicy> => data(await api.GET("/platform/v1/instance/settings/agents")),
});
