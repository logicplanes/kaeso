import { Suspense, lazy, type ReactNode } from "react";
import { useLeaveGoneOrg } from "./rights";
import { useMembership } from "./session";

const loadAccess = () => import("./org-access");
const NeedsTwoFactor = lazy(() => loadAccess().then((m) => ({ default: m.NeedsTwoFactor })));
const NeedsSso = lazy(() => loadAccess().then((m) => ({ default: m.NeedsSso })));
const NeedsOtherSignIn = lazy(() => loadAccess().then((m) => ({ default: m.NeedsOtherSignIn })));
const ScheduledForDeletion = lazy(() => loadAccess().then((m) => ({ default: m.ScheduledForDeletion })));

export function OrgGuard({ org, children }: { org: string; children: ReactNode }) {
  const membership = useMembership(org);
  const gone = useLeaveGoneOrg(org);
  if (gone) return null;
  if (membership?.access === "needs_2fa")
    return (
      <Suspense fallback={null}>
        <NeedsTwoFactor orgName={membership.org_name} />
      </Suspense>
    );
  if (membership?.access === "needs_sso")
    return (
      <Suspense fallback={null}>
        <NeedsSso org={org} orgName={membership.org_name} />
      </Suspense>
    );
  if (membership?.access === "needs_other_sign_in")
    return (
      <Suspense fallback={null}>
        <NeedsOtherSignIn orgName={membership.org_name} />
      </Suspense>
    );
  return (
    <Suspense fallback={null}>
      {membership?.org_state === "scheduled_for_deletion" && <ScheduledForDeletion org={org} />}
      {children}
    </Suspense>
  );
}
