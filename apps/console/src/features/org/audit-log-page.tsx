import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { orgMembersQuery, platformKeysQuery } from "~/api/queries/orgs";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useMembership, useOrgRights } from "~/auth/session";
import type { AuditSearch } from "~/api/queries/audit";
import { t } from "~/i18n";
import { AuditExportButton, AuditLog } from "~/kaeso/audit-log";
import { projectPath } from "~/shell/pages";
import { OrgPage } from "./org-page";
import { useOrgPartyName } from "./party";

const filtersOf = ({ event: _event, ...filters }: AuditSearch) => filters;

export function OrgAuditLogPage({ org, search }: { org: string; search: AuditSearch }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const projects = useQuery(orgProjectsQuery(org, true));
  const credentials = useQuery({ ...orgCredentialsQuery(org), enabled: rights.has("provider_credentials.view") });
  const keys = useQuery({ ...platformKeysQuery(org, true), enabled: rights.has("platform_keys.view") });
  const membership = useMembership(org);
  const partyName = useOrgPartyName(org);
  const base = `/settings/orgs/${org}`;
  const targetHref = (kind: string, id: string): string | undefined => {
    if (kind === "project") return projectPath(org, id);
    if (kind === "user") return `${base}/members?member=${id}`;
    if (kind === "invite") return `${base}/members?tab=invites`;
    if (kind === "credential") return `/orgs/${org}/providers`;
    if (kind === "platform_key") return `${base}/platform-keys`;
    if (kind === "org") return `${base}/general`;
    return undefined;
  };
  const targetName = (kind: string, id: string): string | undefined => {
    if (kind === "project") return projects.data?.find((p) => p.id === id)?.name;
    if (kind === "user") return members.data?.find((m) => m.user.id === id)?.user.name;
    if (kind === "credential") return credentials.data?.find((c) => c.id === id)?.name;
    if (kind === "platform_key") return keys.data?.find((k) => k.id === id)?.name;
    if (kind === "org" && id === org) return membership?.org_name;
    return undefined;
  };
  return (
    <OrgPage
      org={org}
      title={t("org.auditLog.title", "Audit log")}
      hidden={t("org.auditLog.hidden", "You can't see the audit log here")}
      right="audit_log.view"
      description={t("org.auditLog.description", "Every change in the organization: who made it, when and what.")}
      actions={rights.has("audit_log.export") ? <AuditExportButton org={org} filters={filtersOf(search)} /> : undefined}
    >
      <AuditLog
        scope={{ org }}
        search={search}
        onSearch={(next) => void navigate({ to: "/settings/orgs/$org/audit-log", params: { org }, search: next, replace: true })}
        partyName={partyName}
        actors={(members.data ?? []).map((m) => ({ id: m.user.id, name: m.user.name, email: m.user.email }))}
        targetHref={targetHref}
        targetName={targetName}
      />
    </OrgPage>
  );
}
