import { useEffect, useReducer, useRef } from "react";
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import type { MemberUser } from "~/api/generated/types";
import { t } from "~/i18n";
import { grantorsQuery, projectGrantorsQuery, rightsCatalogueQuery } from "~/api/queries/rights";
import { indexCatalogue, type Catalogue } from "~/kaeso/catalogue";
import { eitherOf } from "./people";
import { useReadOnly } from "./rights";
import { orgRights, sessionQuery } from "./session";

export function needsText(name: string, grantors: readonly Pick<MemberUser, "name">[] | undefined): string {
  const [first, second] = (grantors ?? []).map((g) => g.name);
  if (first === undefined) return t("auth.needsRight.ownerOrAdmin", "You need {right}; ask an owner or admin for it.", { right: name });
  return t("auth.needsRight.ask", "You need {right}; ask {person} for it.", { right: name, person: second === undefined ? first : eitherOf(first, second) });
}

function grantorsFor(cat: Catalogue, right: string, org: string, project: string | undefined, canSeeMembers: boolean) {
  const scope = cat.byId.get(right)?.scope;
  if (scope === "project") return project ? projectGrantorsQuery(org, project, right) : null;
  return canSeeMembers ? grantorsQuery(org, right) : null;
}

export async function explainMissingRight(queryClient: QueryClient, org: string, project: string | undefined, right: string): Promise<string> {
  try {
    const [catalogue, session] = await Promise.all([queryClient.fetchQuery(rightsCatalogueQuery(org)), queryClient.ensureQueryData(sessionQuery)]);
    const cat = indexCatalogue(catalogue);
    const name = cat.byId.get(right)?.name ?? right;
    const query = grantorsFor(cat, right, org, project, orgRights(session, org).has("members.view"));
    const grantors = query ? await queryClient.fetchQuery(query).catch(() => undefined) : undefined;
    return needsText(name, grantors);
  } catch {
    return needsText(right, undefined);
  }
}

export function useMissingRight(org: string, project: string | undefined, readOnly: boolean): (right: string) => string {
  const queryClient = useQueryClient();
  const cat = useQuery({ ...rightsCatalogueQuery(org), select: indexCatalogue, enabled: !!org }).data;
  const asked = useRef(new Set<string>());
  const [, rerender] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (!cat || readOnly) return;
    const session = queryClient.getQueryData(sessionQuery.queryKey);
    const canSeeMembers = !!session && orgRights(session, org).has("members.view");
    for (const right of asked.current) {
      const query = grantorsFor(cat, right, org, project, canSeeMembers);
      if (!query || queryClient.getQueryState(query.queryKey)) continue;
      void queryClient
        .fetchQuery(query)
        .catch(() => undefined)
        .finally(rerender);
    }
  });
  return (right) => {
    if (readOnly) return t("auth.readOnly", "The organization is scheduled for deletion");
    asked.current.add(right);
    const name = cat?.byId.get(right)?.name ?? right;
    const query = cat ? grantorsFor(cat, right, org, project, true) : null;
    return needsText(name, query ? queryClient.getQueryData(query.queryKey) : undefined);
  };
}

export function useNeedsRight(org: string, project?: string): (right: string) => string {
  return useMissingRight(org, project, useReadOnly(org));
}
