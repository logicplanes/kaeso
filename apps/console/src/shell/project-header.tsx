import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppFrame, PageHeader } from "@logicplanes/kaeso-ui";
import { projectQuery } from "~/api/queries/projects";
import { t } from "~/i18n";
import { usePageTitle } from "./title";

export function ProjectHeader({
  org,
  project,
  page,
  tabs,
  actions,
}: {
  org: string;
  project: string;
  page: string;
  path?: string;
  tabs?: ReactNode;
  actions?: ReactNode;
}) {
  const current = useQuery(projectQuery(org, project));
  usePageTitle(page, current.data?.name ?? t("common.project", "Project"));
  return <PageHeader start={<AppFrame.MenuButton />} title={page} {...(tabs ? { tabs } : {})} {...(actions ? { actions } : {})} />;
}
