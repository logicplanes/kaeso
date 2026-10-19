import type { InvalidateQueryFilters, Query, QueryKey } from "@tanstack/react-query";

export type QueryMatch = (query: Query) => boolean;

export type Invalidation = QueryKey | QueryMatch;

export function invalidationFilters(invalidation: Invalidation): InvalidateQueryFilters {
  return typeof invalidation === "function" ? { predicate: invalidation } : { queryKey: invalidation };
}

const projectPart = (key: QueryKey, part: string, org?: string): boolean =>
  key[0] === "orgs" && (org === undefined || key[1] === org) && key[2] === "projects" && key[4] === part;

export const modelCatalogues: QueryMatch = ({ queryKey }) => ["models", "aliases", "playground"].some((part) => projectPart(queryKey, part));

export const memberViews: QueryMatch = ({ queryKey }) => (queryKey[0] === "orgs" && queryKey[2] === "members") || projectPart(queryKey, "members");

export const projectViews =
  (org: string): QueryMatch =>
  ({ queryKey }) =>
    projectPart(queryKey, "view", org);

export const projectLogSettings =
  (org: string): QueryMatch =>
  ({ queryKey }) =>
    projectPart(queryKey, "logs", org) && queryKey[5] === "settings";
