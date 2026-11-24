import { Suspense, lazy, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { DialogScope } from "@logicplanes/kaeso-ui";
import { securityQuery } from "~/api/queries/orgs";
import { useNeedsRight } from "~/auth/needs-right";
import { useMembership, useOrgRights } from "~/auth/session";
import { orgProjectsQuery } from "~/api/queries/projects";
import { t } from "~/i18n";
import { ShareTrigger } from "./share";

const ShareDialog = lazy(() => import("./share-dialog"));

export type ShareKind = "analytics" | "member" | "budget" | "project_card";

export interface ShareTarget {
  kind: ShareKind;
  /** The project shared; the whole organization without one. */
  project?: string | undefined;
  /** The member whose stats are shared. */
  member?: string | undefined;
  /** The member the default title names; a project's or the organization's name otherwise. */
  scope?: string | undefined;
  from: string;
  to: string;
  /** Whether the person sees the costs here, so the link may show them. */
  costs: boolean;
}

export const EXPIRIES = [1, 7, 30, 90] as const;
export const NEVER = "never" as const;

export function expiryLabel(d: number | typeof NEVER): string {
  return d === NEVER ? t("kaeso.share.never", "Never") : t("kaeso.share.afterDays", "After {count, plural, one {# day} other {# days}}", { count: d });
}

/** Opens the dialog that shares what the page shows as a read-only link; `compact` leaves only its icon on a phone, for a crowded header. */
export function ShareButton({ org, target, compact = false }: { org: string; target: ShareTarget; compact?: boolean | undefined }) {
  const [open, setOpen] = useState(false);
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const policy = useQuery({ ...securityQuery(org), enabled: rights.has("usage.share") }).data?.shared_links;
  const projects = useQuery({ ...orgProjectsQuery(org), enabled: target.scope === undefined }).data;
  const orgName = useMembership(org)?.org_name;
  const scope = target.scope ?? (target.project ? projects?.find((p) => p.id === target.project)?.name : orgName) ?? "";
  const reason = !rights.has("usage.share")
    ? needs("usage.share")
    : policy?.allow === "off"
      ? t("kaeso.share.off", "This organization doesn't allow shared links; ask one of its admins")
      : undefined;
  return (
    <>
      <ShareTrigger compact={compact} disabled={!!reason || !policy} disabledReason={reason} onClick={() => setOpen(true)} />
      <DialogScope open={open}>
        {open && policy && (
          <Suspense fallback={null}>
            <ShareDialog
              org={org}
              target={{ ...target, scope }}
              anyone={policy.allow === "anyone"}
              maxDays={policy.max_days ?? null}
              onClose={() => setOpen(false)}
            />
          </Suspense>
        )}
      </DialogScope>
    </>
  );
}
