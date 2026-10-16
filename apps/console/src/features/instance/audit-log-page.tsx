import { keepPreviousData, useInfiniteQuery, useQueries } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useCallback, useMemo, useState } from "react";
import { auditLogQuery, type AuditSearch } from "~/api/queries/audit";
import { instanceOrgQuery, instanceUserQuery, instanceUsersQuery } from "~/api/queries/instance";
import { useSession } from "~/auth/session";
import { AuditLog } from "~/kaeso/audit-log";
import type { PartyLabel, PartyRef } from "~/kaeso/party";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

const unique = (ids: (string | null | undefined)[]): string[] => [...new Set(ids.filter((id): id is string => !!id))];
const loaded = <T,>(results: { data?: T | undefined }[]): T[] => results.flatMap((r) => (r.data ? [r.data] : []));

export function InstanceAuditLogPage({ search }: { search: AuditSearch }) {
  const navigate = useNavigate();
  const session = useSession();
  const { event: _event, ...filters } = search;
  const [actorQuery, setActorQuery] = useState("");
  const found = useInfiniteQuery({ ...instanceUsersQuery(actorQuery, undefined), placeholderData: keepPreviousData });
  const log = useInfiniteQuery(auditLogQuery({ instance: true }, filters));
  const events = useMemo(() => log.data?.pages.flatMap((p) => p.data) ?? [], [log.data]);
  const matches = useMemo(() => found.data?.pages.flatMap((p) => p.data) ?? [], [found.data]);
  const userIds = unique([
    filters.actor,
    ...events.flatMap((e) => [e.actor.kind === "user" ? e.actor.id : null, e.target_kind === "user" ? e.target_id : null]),
  ]);
  const orgIds = unique(events.map((e) => (e.target_kind === "org" ? e.target_id : null)));
  const users = useQueries({ queries: userIds.map((id) => ({ ...instanceUserQuery(id), meta: { quiet: true } })), combine: loaded });
  const orgs = useQueries({ queries: orgIds.map((id) => ({ ...instanceOrgQuery(id), meta: { quiet: true } })), combine: loaded });
  const known = useMemo(() => new Map([...matches, ...users].map((u) => [u.id, u])), [matches, users]);
  const orgNames = new Map(orgs.map((o) => [o.id, o.name]));
  const targetName = (kind: string, id: string) => (kind === "user" ? known.get(id)?.name : kind === "org" ? orgNames.get(id) : undefined);
  const partyLabel = useCallback(
    (p: PartyRef): PartyLabel => {
      if (p.kind === "system") return { name: t("instance.system", "System"), you: false };
      if (p.kind === "user" && p.id === session.user.id) return { name: session.user.name, you: true };
      if (p.kind === "user") return { name: known.get(p.id ?? "")?.name ?? p.id ?? t("instance.aUser", "A user"), you: false };
      return { name: p.id ?? p.kind, you: false };
    },
    [known, session.user.id, session.user.name],
  );
  const selected = filters.actor ? known.get(filters.actor) : undefined;
  const actors = [...(selected && !matches.some((u) => u.id === selected.id) ? [selected] : []), ...matches];
  return (
    <InstancePage
      title={t("instance.auditLog", "Audit log")}
      description={t("instance.auditLogHint", "Changes to users, organizations and the install's settings.")}
    >
      <AuditLog
        scope={{ instance: true }}
        search={search}
        onSearch={(next) => void navigate({ to: "/settings/instance/audit-log", search: next, replace: true })}
        partyLabel={partyLabel}
        actors={actors.map((u) => ({ id: u.id, name: u.name, email: u.email }))}
        onActorSearch={setActorQuery}
        actorsLoading={found.isFetching}
        targetName={targetName}
        targetHref={(kind, id) => (kind === "user" ? `/settings/instance/users?user=${id}` : kind === "org" ? `/settings/instance/orgs/${id}` : undefined)}
      />
    </InstancePage>
  );
}
