import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Badge, Button, Combobox, Dialog, Empty, List, Text, icons } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { ProjectMemberView } from "~/api/generated/types";
import { projectMembersQuery } from "~/api/queries/members";
import { orgMembersQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights, useSession } from "~/auth/session";
import { userStateLabel } from "~/format/labels";
import { useCatalogue } from "~/kaeso/catalogue";
import { grantLabel, type Grant } from "~/kaeso/grant";
import { GrantEditor } from "~/kaeso/rights-editor";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { MemberSheet } from "./member-sheet";
import { LoadFailed } from "~/features/org/load-failed";
import { useProjectRights } from "~/auth/rights";
import { useProjectGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { compareNames } from "~/format/collate";
import { common } from "~/i18n/common";
import { partyText } from "~/kaeso/party";
import { patchSearch, type SearchPatch } from "~/search";

export type MembersSearch = { member?: string; add?: boolean };

export function MembersPage({ org, project, search }: { org: string; project: string; search: MembersSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const session = useSession();
  const rights = useProjectRights(org, project);
  const members = useQuery(projectMembersQuery(org, project));
  const [adding, setAdding] = useState(!!search.add);
  const canAdd = rights.has("project.members.add");
  const reason = canAdd ? undefined : needs("project.members.add");
  usePageCreateAction({ label: t("project.members.add", "Add member"), run: () => setAdding(true), disabledReason: reason });

  const setSearch = (patch: SearchPatch<MembersSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/members", params: { org, project }, search: patchSearch(search, patch, ["add"]), replace: true });
  };

  const list = useMemo(() => [...(members.data ?? [])].sort((a, b) => compareNames(a.user.name, b.user.name)), [members.data]);
  const items = list.map((m: ProjectMemberView) => ({
    id: m.user.id,
    textValue: m.user.name,
    title: partyText({ name: m.user.name, you: m.user.id === session.user.id }),
    subtitle: m.user.email ?? undefined,
    leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
    meta: (
      <span className="flex items-center gap-8">
        {m.user.state !== "active" && <Badge tone={m.user.state === "deleted" ? "neutral" : "warning"}>{userStateLabel(m.user.state)}</Badge>}
        <Badge>{grantLabel(m.grant)}</Badge>
      </span>
    ),
    date: formatDate(m.joined_at),
  }));

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={common.members()}
        path="members"
        actions={
          <Button
            size="sm"
            variant="primary"
            leadingIcon={icons.add}
            disabled={!canAdd}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => setAdding(true)}
          >
            {t("project.members.add", "Add member")}
          </Button>
        }
      />
      <AppFrame.Body padding="list">
        {members.isPending ? (
          <List aria-label={common.members()} items={[]} loading />
        ) : members.isError ? (
          <LoadFailed title={t("project.members.listFailed", "The members couldn't load")} size="md" />
        ) : list.length === 0 ? (
          <Empty
            size="lg"
            title={t("project.members.none", "No members")}
            description={t("project.members.noneHint", "Add organization members to give them rights in this project.")}
          />
        ) : (
          <List aria-label={common.members()} items={items} onOpen={(item) => setSearch({ member: item.id })} />
        )}
      </AppFrame.Body>
      <MemberSheet org={org} project={project} userId={search.member} onClose={() => setSearch({ member: undefined })} />
      {adding && <AddMemberDialog org={org} project={project} existing={new Set(list.map((m) => m.user.id))} onClose={() => setAdding(false)} />}
    </>
  );
}

function AddMemberDialog({ org, project, existing, onClose }: { org: string; project: string; existing: Set<string>; onClose: () => void }) {
  const orgRights = useOrgRights(org);
  const needs = useNeedsRight(org, project);
  const giver = useProjectGiver(org, project);
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const orgMembers = useQuery({ ...orgMembersQuery(org), enabled: orgRights.has("members.view") });
  const [user, setUser] = useState<string | null>(null);
  const [grant, setGrant] = useState<Grant>({ role: "developer", rights_added: [], rights_removed: [] });
  const candidates = (orgMembers.data ?? []).filter((m) => !existing.has(m.user.id) && m.user.state === "active");

  const add = useApiMutation({
    mutationFn: async (userId: string) =>
      data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/members", { params: { path: { org, project } }, body: { user_id: userId, ...grant } })),
    invalidates: [["orgs", org, "projects", project, "members"]],
    onSuccess: (m) => {
      toast(t("project.members.added", "{name} added to the project", { name: m.user.name }));
      onClose();
    },
  });

  const submit = () => user && add.mutate(user);

  return (
    <Dialog
      busy={add.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("project.members.add", "Add member")}
      description={t("project.members.addHint", "Add someone who is already in the organization; invite others to the organization first.")}
      onSubmit={submit}
      footer={
        <>
          <Dialog.Close>
            <Button>{common.cancel()}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={!user} loading={add.isPending} onClick={submit}>
            {t("project.members.add", "Add member")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-20">
        {!orgRights.has("members.view") ? (
          <Text variant="small" tone="secondary">
            {t("project.members.needsOrgMembers", "Choosing a member means seeing the organization's members. {reason}", { reason: needs("members.view") })}
          </Text>
        ) : !orgMembers.isPending && !candidates.length ? (
          <Text variant="small" tone="secondary">
            {t("project.members.allIn", "Every member is in this project")}
          </Text>
        ) : (
          <Combobox
            label={t("project.members.member", "Member")}
            placeholder={orgMembers.isPending ? t("project.members.loading", "Loading members…") : t("project.members.choose", "Choose a member")}
            options={candidates.map((m) => ({
              value: m.user.id,
              label: m.user.name,
              description: m.user.email ?? undefined,
              leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
            }))}
            value={user}
            onValueChange={(v: string | null) => setUser(v)}
            disabled={!candidates.length}
          />
        )}
        <GrantEditor cat={cat} scope="project" roles={rolesOfScope(roles, "project")} value={grant} onChange={setGrant} giver={giver} labelled />
      </div>
    </Dialog>
  );
}
