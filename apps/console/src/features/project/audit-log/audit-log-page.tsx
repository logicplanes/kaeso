import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame } from "@logicplanes/kaeso-ui";
import type { AuditSearch } from "~/api/queries/audit";
import { projectMembersQuery } from "~/api/queries/members";
import { useProjectRights } from "~/auth/rights";
import { AuditLog } from "~/kaeso/audit-log";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyLabel } from "../keys/party";
import { t } from "~/i18n";

export function ProjectAuditLogPage({ org, project, search }: { org: string; project: string; search: AuditSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const partyLabel = usePartyLabel(org, project);
  const base = `/orgs/${org}/projects/${project}`;
  const targetHref = useCallback((kind: string, id: string) => (kind === "user" ? `${base}/members?member=${id}` : undefined), [base]);
  const targetName = useCallback(
    (kind: string, id: string) => (kind === "user" ? members.data?.find((m) => m.user.id === id)?.user.name : undefined),
    [members.data],
  );
  return (
    <>
      <ProjectHeader org={org} project={project} page={t("project.auditLog.title", "Audit log")} path="audit-log" />
      <AppFrame.Body padding="list">
        <AuditLog
          scope={{ org, project }}
          search={search}
          onSearch={(next) => void navigate({ to: "/orgs/$org/projects/$project/audit-log", params: { org, project }, search: next, replace: true })}
          partyLabel={partyLabel}
          actors={(members.data ?? []).map((m) => m.user)}
          targetHref={targetHref}
          targetName={targetName}
        />
      </AppFrame.Body>
    </>
  );
}
