import { useMemo } from "react";
import { queryOptions, useQuery, useSuspenseQuery, type QueryClient } from "@tanstack/react-query";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { Membership, MyMembership, WhoAmI } from "~/api/generated/types";
import { membershipsQuery, sessionQuery } from "./session-queries";

export { membershipsQuery, sessionQuery };

export function useSession(): WhoAmI {
  return useSuspenseQuery(sessionQuery).data;
}

export function useMemberships(): MyMembership[] {
  return useSuspenseQuery(membershipsQuery).data;
}

export function useRequestLogs(): boolean {
  return useSession().request_logs;
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

export interface ProjectAccess {
  rights: ReadonlySet<string>;
  /** Whether the person is a member of the project, rather than reaching it through an organization-wide role. */
  member: boolean;
}

export function projectRightsQuery(session: WhoAmI, org: string, project: string) {
  return queryOptions({
    queryKey: [...PROJECT_RIGHTS_KEY, session.user.id, org, project],
    queryFn: async (): Promise<ProjectAccess> => {
      try {
        const mine = data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/my-rights", { params: { path: { org, project } } }));
        return { rights: new Set(mine.rights.map((r) => r.right)), member: mine.role !== null };
      } catch (error) {
        if (isApiError(error) && (error.type === "not_found" || error.type === "permission_denied"))
          return { rights: new Set(everyProjectRights(session, org)), member: false };
        throw error;
      }
    },
    staleTime: 60_000,
  });
}

const NONE: ReadonlySet<string> = new Set();

/**
 * The organization rights a route loader can rely on: the session the app's route loaded first. Without it, none, so a
 * loader never asks for something the page itself wouldn't.
 */
export function loadedOrgRights(client: QueryClient, org: string): ReadonlySet<string> {
  const session = client.getQueryData(sessionQuery.queryKey);
  return session ? orgRights(session, org) : NONE;
}

export function useProjectRights(org: string, project: string): ReadonlySet<string> {
  const session = useSession();
  return useQuery({ ...projectRightsQuery(session, org, project), enabled: !!org && !!project, select: (a) => a.rights }).data ?? NONE;
}

/** Whether you are a member of the project; undefined until your rights there are known. */
export function useProjectMember(org: string, project: string): boolean | undefined {
  const session = useSession();
  return useQuery({ ...projectRightsQuery(session, org, project), enabled: !!org && !!project, select: (a) => a.member }).data;
}
