import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { AuditEventView } from "../generated/types";
import { endOfDay } from "~/format/period";
import type { AuditFilters } from "./audit-search";
import { nextPageParam, refetchOnFocusWhileOnePage } from "../paging";

export type { AuditFilters, AuditSearch } from "./audit-search";

export type AuditScope = { org: string; project?: string | undefined } | { instance: true };

export function auditParams(f: AuditFilters) {
  return {
    ...(f.actor ? { actor: f.actor } : {}),
    ...(f.action ? { action: f.action } : {}),
    ...(f.target ? { target: f.target } : {}),
    ...(f.by_agents ? { by_agents: true } : {}),
    ...(f.from ? { from: `${f.from}T00:00:00Z` } : {}),
    ...(f.to ? { to: endOfDay(f.to) } : {}),
  };
}

const scopeKey = (scope: AuditScope) => ("org" in scope ? ["org", scope.org, ...(scope.project ? ["project", scope.project] : [])] : ["instance"]);

export const auditLogQuery = (scope: AuditScope, filters: AuditFilters) =>
  infiniteQueryOptions({
    queryKey: ["audit-log", ...scopeKey(scope), filters],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam, signal }) => {
      const query = { ...auditParams(filters), limit: 100, ...(pageParam ? { after: pageParam } : {}) };
      if ("org" in scope && scope.project)
        return data(
          await api.GET("/platform/v1/orgs/{org}/projects/{project}/audit-log", {
            params: { path: { org: scope.org, project: scope.project }, query },
            signal,
          }),
        );
      return "org" in scope
        ? data(await api.GET("/platform/v1/orgs/{org}/audit-log", { params: { path: { org: scope.org }, query }, signal }))
        : data(await api.GET("/platform/v1/instance/audit-log", { params: { query }, signal }));
    },
    getNextPageParam: nextPageParam,
    refetchOnWindowFocus: refetchOnFocusWhileOnePage,
  });

export const auditEventQuery = (scope: AuditScope, event: string) =>
  queryOptions({
    queryKey: ["audit-log", ...scopeKey(scope), "one", event],
    queryFn: async (): Promise<AuditEventView> =>
      "org" in scope
        ? data(await api.GET("/platform/v1/orgs/{org}/audit-log/{event}", { params: { path: { org: scope.org, event } } }))
        : data(await api.GET("/platform/v1/instance/audit-log/{event}", { params: { path: { event } } })),
  });
