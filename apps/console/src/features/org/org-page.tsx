import { Suspense, lazy, type ReactNode } from "react";
import { Empty } from "@logicplanes/kaeso-ui";
import { useLeaveGoneOrg } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { useMembership, useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { SettingsPage } from "~/shell/settings-page";

const ScheduledForDeletion = lazy(() => import("~/auth/org-access").then((m) => ({ default: m.ScheduledForDeletion })));

export function OrgPage({
  org,
  title,
  hidden,
  right,
  description,
  actions,
  children,
}: {
  org: string;
  title: string;
  hidden: string;
  right: string;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const membership = useMembership(org);
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const gone = useLeaveGoneOrg(org);
  const context = membership?.org_name ?? t("common.organization", "Organization");
  if (gone) return null;
  if (!rights.has(right)) {
    return (
      <SettingsPage title={title} context={context}>
        <Empty title={hidden} description={needs(right)} />
      </SettingsPage>
    );
  }
  return (
    <SettingsPage title={title} context={context} {...(description ? { description } : {})} {...(actions ? { actions } : {})}>
      <Suspense fallback={null}>
        {membership?.org_state === "scheduled_for_deletion" && <ScheduledForDeletion org={org} layout="inline" />}
        {children}
      </Suspense>
    </SettingsPage>
  );
}
