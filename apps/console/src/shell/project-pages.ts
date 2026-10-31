import { useMemo } from "react";
import { useProjectRights } from "~/auth/rights";
import { useRequestLogs } from "~/auth/session";
import { PROJECT_PAGES, type ProjectPage } from "./pages";

export function useProjectPages(org: string, project: string): ProjectPage[] {
  const rights = useProjectRights(org, project);
  const requestLogs = useRequestLogs();
  return useMemo(() => PROJECT_PAGES.filter((p) => rights.has(p.right) && (requestLogs || p.id !== "logs")), [rights, requestLogs]);
}
