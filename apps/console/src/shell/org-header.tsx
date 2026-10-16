import type { ReactNode } from "react";
import { AppFrame, PageHeader } from "@logicplanes/kaeso-ui";
import { useMembership } from "~/auth/session";
import { usePageTitle } from "./title";

export function OrgHeader({ org, page, tabs, actions }: { org: string; page: string; tabs?: ReactNode; actions?: ReactNode }) {
  const membership = useMembership(org);
  usePageTitle(page, membership?.org_name);
  return <PageHeader start={<AppFrame.MenuButton />} title={page} {...(tabs ? { tabs } : {})} {...(actions ? { actions } : {})} />;
}
