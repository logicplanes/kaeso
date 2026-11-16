import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame } from "@logicplanes/kaeso-ui";
import type { AuditSearch } from "~/api/queries/audit";
import { allKeysQuery } from "~/api/queries/keys";
import { limitsQuery } from "~/api/queries/limits";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { useProjectRights } from "~/auth/rights";
import { modelName } from "~/format/models";
import { AuditLog } from "~/kaeso/audit-log";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyLabel } from "../keys/party";
import { t } from "~/i18n";

interface Target {
  name: string | undefined;
  href: string;
}

export function ProjectAuditLogPage({ org, project, search }: { org: string; project: string; search: AuditSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const models = useQuery({ ...catalogueQuery(org, project), enabled: rights.has("models.view") });
  const aliases = useQuery({ ...aliasesQuery(org, project), enabled: rights.has("models.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  const limits = useQuery({ ...limitsQuery(org, project), enabled: rights.has("limits.view") });
  const partyLabel = usePartyLabel(org, project);
  const base = `/orgs/${org}/projects/${project}`;
  // A target the project still has, with where it opens; one that is gone, or that the person can't see, keeps the
  // name from its event's snapshot and no link.
  const target = useCallback(
    (kind: string, id: string): Target | undefined => {
      const find = <T extends { id: string }>(list: T[] | undefined) => list?.find((x) => x.id === id);
      switch (kind) {
        case "user": {
          const m = members.data?.find((x) => x.user.id === id);
          return m && { name: m.user.name, href: `${base}/members?member=${id}` };
        }
        case "key": {
          const k = find(keys.data);
          return k && { name: k.name, href: `${base}/keys?key=${id}` };
        }
        case "catalogue_model": {
          const m = find(models.data);
          return m && { name: modelName(m), href: `${base}/models?model=${id}` };
        }
        case "alias": {
          const a = find(aliases.data);
          return a && { name: a.name, href: `${base}/models/aliases/${id}` };
        }
        case "service_account": {
          const a = find(accounts.data);
          return a && { name: a.name, href: `${base}/service-accounts?account=${id}` };
        }
        case "limit": {
          const l = find(limits.data);
          return l && { name: l.description || undefined, href: `${base}/limits?level=${l.level}` };
        }
        case "portal_grant":
          return rights.has("tools.view") ? { name: undefined, href: `${base}/tools` } : undefined;
        case "project":
          return id === project && rights.has("project.view") ? { name: undefined, href: `${base}/settings` } : undefined;
        default:
          return undefined;
      }
    },
    [base, project, rights, members.data, keys.data, models.data, aliases.data, accounts.data, limits.data],
  );
  const targetHref = useCallback((kind: string, id: string) => target(kind, id)?.href, [target]);
  const targetName = useCallback((kind: string, id: string) => target(kind, id)?.name, [target]);
  return (
    <>
      <ProjectHeader org={org} project={project} page={t("project.auditLog.title", "Audit log")} path="audit-log" />
      <AppFrame.Body padding="none">
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
