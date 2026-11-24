import { useEffect, useMemo, useRef } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import type { WhoAmI } from "~/api/generated/types";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import { membershipsQuery, projectRightsQuery, sessionQuery, useMembership, useProjectRights as useGrantedProjectRights } from "./session";

export const keptWhileReadOnly = (right: string): boolean => /\.view\b|\.export$/.test(right);

export const useReadOnly = (org: string): boolean => useMembership(org)?.org_state === "scheduled_for_deletion";

export function useProjectRights(org: string, project: string): ReadonlySet<string> {
  const rights = useGrantedProjectRights(org, project);
  const readOnly = useReadOnly(org);
  return useMemo(() => (readOnly ? new Set([...rights].filter(keptWhileReadOnly)) : rights), [rights, readOnly]);
}

/**
 * The project rights a route loader can rely on, as `useProjectRights` reads them: those the project's gate loaded
 * before any page's loader runs, less what a read-only organization takes away. None without them.
 */
export function loadedProjectRights(client: QueryClient, org: string, project: string): ReadonlySet<string> {
  const session = client.getQueryData(sessionQuery.queryKey);
  const rights = session && client.getQueryData(projectRightsQuery(session, org, project).queryKey)?.rights;
  if (!session || !rights) return new Set();
  const readOnly = session.memberships.find((m) => m.org_id === org)?.org_state === "scheduled_for_deletion";
  return readOnly ? new Set([...rights].filter(keptWhileReadOnly)) : rights;
}

export function useLeaveGoneOrg(org: string): boolean {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const membership = useMembership(org);
  const seen = useRef<{ name: string; scheduled: boolean } | null>(null);
  const checking = useRef(false);
  useEffect(() => {
    if (membership) {
      seen.current = { name: membership.org_name, scheduled: membership.org_state === "scheduled_for_deletion" };
      return;
    }
    if (checking.current) return;
    checking.current = true;
    void (async () => {
      const session = await queryClient.fetchQuery({ ...sessionQuery, staleTime: 0 }).catch(() => null as WhoAmI | null);
      checking.current = false;
      if (!session || session.memberships.some((m) => m.org_id === org)) return;
      const was = seen.current;
      toast(
        !was
          ? t("auth.gone.notMember", "That organization doesn't exist or you can't access it")
          : was.scheduled
            ? t("auth.gone.deleted", "{org} was deleted", { org: was.name })
            : t("auth.gone.left", "You no longer belong to {org}", { org: was.name }),
      );
      await queryClient.invalidateQueries({ queryKey: membershipsQuery.queryKey });
      await navigate({ to: "/", replace: true });
    })();
  }, [membership, org, navigate, queryClient]);
  return !membership;
}
