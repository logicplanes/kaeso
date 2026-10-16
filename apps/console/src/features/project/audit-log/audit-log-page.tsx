import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame } from "@logicplanes/kaeso-ui";
import type { Party } from "~/api/generated/types";
import type { AuditSearch } from "~/api/queries/audit";
import { projectMembersQuery } from "~/api/queries/members";
import { useProjectRights } from "~/auth/rights";
import { AuditLog } from "~/kaeso/audit-log";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyName } from "../keys/party";
import { t } from "~/i18n";

export function ProjectAuditLogPage({ org, project, search }: { org: string; project: string; search: AuditSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const partyName = usePartyName(org, project);
  const base = `/orgs/${org}/projects/${project}`;
  return (
    <>
      <ProjectHeader org={org} project={project} page={t("project.auditLog.title", "Audit log")} path="audit-log" />
      <AppFrame.Body padding="list">
        <AuditLog
          scope={{ org, project }}
          search={search}
          onSearch={(next) => void navigate({ to: "/orgs/$org/projects/$project/audit-log", params: { org, project }, search: next, replace: true })}
          partyName={(p) => partyName(p as Party)}
          actors={(members.data ?? []).map((m) => ({ id: m.user.id, name: m.user.name, email: m.user.email }))}
          targetHref={(kind, id) => (kind === "user" ? `${base}/members?member=${id}` : undefined)}
          targetName={(kind, id) => (kind === "user" ? members.data?.find((m) => m.user.id === id)?.user.name : undefined)}
        />
      </AppFrame.Body>
    </>
  );
}
