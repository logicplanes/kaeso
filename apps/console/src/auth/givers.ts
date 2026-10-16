import { useMemo } from "react";
import type { Giver } from "~/kaeso/roles";
import { useProjectRights } from "./rights";
import { everyProjectRights, orgRights, useMembership, useSession } from "./session";

export function useOrgGiver(org: string): Giver {
  const session = useSession();
  const membership = useMembership(org);
  return useMemo(
    () => ({ held: orgRights(session, org), everyProject: everyProjectRights(session, org), isOwner: membership?.role.id === "owner" }),
    [session, org, membership],
  );
}

export function useProjectGiver(org: string, project: string): Giver {
  const held = useProjectRights(org, project);
  const membership = useMembership(org);
  return useMemo(() => ({ held, isOwner: membership?.role.id === "owner" }), [held, membership]);
}
