import { useMemo } from "react";
import { queryOptions, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { Membership, MyMembership, WhoAmI } from "~/api/generated/types";

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: async (): Promise<WhoAmI> => data(await api.GET("/platform/v1/auth/session")),
  staleTime: 0,
});

export const membershipsQuery = queryOptions({
  queryKey: ["me", "memberships"],
  queryFn: async (): Promise<MyMembership[]> => data(await api.GET("/platform/v1/me/memberships")).data,
});

export function useSession(): WhoAmI {
  return useSuspenseQuery(sessionQuery).data;
}

export function useMemberships(): MyMembership[] {
  return useSuspenseQuery(membershipsQuery).data;
}

export function useMembership(org: string): Membership | undefined {
  return useSession().memberships.find((m) => m.org_id === org);
}

export function orgRights(session: WhoAmI, org: string): ReadonlySet<string> {
  return new Set(session.memberships.find((m) => m.org_id === org)?.rights ?? []);
}

export function useOrgRights(org: string): ReadonlySet<string> {
  const session = useSession();
  return useMemo(() => orgRights(session, org), [session, org]);
}

export function everyProjectRights(session: WhoAmI, org: string): ReadonlySet<string> {
  return new Set(session.memberships.find((m) => m.org_id === org)?.project_rights ?? []);
}

export const PROJECT_RIGHTS_KEY = ["project-rights"] as const;

export function projectRightsQuery(session: WhoAmI, org: string, project: string) {
  return queryOptions({
    queryKey: [...PROJECT_RIGHTS_KEY, session.user.id, org, project],
    queryFn: async (): Promise<ReadonlySet<string>> => {
      try {
        const mine = data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/my-rights", { params: { path: { org, project } } }));
        return new Set(mine.rights.map((r) => r.right));
      } catch (error) {
        if (isApiError(error) && (error.type === "not_found" || error.type === "permission_denied")) return new Set(everyProjectRights(session, org));
        throw error;
      }
    },
    staleTime: 60_000,
  });
}

const NONE: ReadonlySet<string> = new Set();

export function useProjectRights(org: string, project: string): ReadonlySet<string> {
  const session = useSession();
  return useQuery({ ...projectRightsQuery(session, org, project), enabled: !!org && !!project }).data ?? NONE;
}
