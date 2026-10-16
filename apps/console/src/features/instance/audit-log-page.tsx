import { useInfiniteQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useCallback, useMemo } from "react";
import type { AuditSearch } from "~/api/queries/audit";
import { instanceOrgsQuery, instanceUsersQuery } from "~/api/queries/instance";
import { useSession } from "~/auth/session";
import { AuditLog } from "~/kaeso/audit-log";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

export function InstanceAuditLogPage({ search }: { search: AuditSearch }) {
  const navigate = useNavigate();
  const session = useSession();
  const users = useInfiniteQuery(instanceUsersQuery("", undefined));
  const orgs = useInfiniteQuery(instanceOrgsQuery(""));
  const list = useMemo(() => users.data?.pages.flatMap((p) => p.data) ?? [], [users.data]);
  const orgList = useMemo(() => orgs.data?.pages.flatMap((p) => p.data) ?? [], [orgs.data]);
  const targetName = useCallback(
    (kind: string, id: string) => (kind === "user" ? list.find((u) => u.id === id)?.name : kind === "org" ? orgList.find((o) => o.id === id)?.name : undefined),
    [list, orgList],
  );
  const partyName = useCallback(
    (p: { kind: string; id?: string | null | undefined }) => {
      if (p.kind === "system") return t("instance.system", "System");
      if (p.kind === "user")
        return p.id === session.user.id
          ? t("instance.nameYou", "{name} (you)", { name: session.user.name })
          : (list.find((u) => u.id === p.id)?.name ?? p.id ?? t("instance.aUser", "A user"));
      return p.id ?? p.kind;
    },
    [list, session.user.id, session.user.name],
  );
  return (
    <InstancePage
      title={t("instance.auditLog", "Audit log")}
      description={t("instance.auditLogHint", "Changes to users, organizations and the install's settings.")}
    >
      <AuditLog
        scope={{ instance: true }}
        search={search}
        onSearch={(next) => void navigate({ to: "/settings/instance/audit-log", search: next, replace: true })}
        partyName={partyName}
        actors={list.map((u) => ({ id: u.id, name: u.name, email: u.email }))}
        targetName={targetName}
        targetHref={(kind, id) => (kind === "user" ? `/settings/instance/users?user=${id}` : kind === "org" ? `/settings/instance/orgs/${id}` : undefined)}
      />
    </InstancePage>
  );
}
