import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Avatar, Badge, Button, Empty, KeyValue, List, Section, Sheet, Skeleton, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { MemberView } from "~/api/generated/types";
import { orgMembersQuery } from "~/api/queries/orgs";
import { memberRightsQuery } from "~/api/queries/rights";
import { useApiMutation } from "~/auth/mutation";
import { useOrgGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { everyProjectRights, useMembership, useOrgRights, useSession } from "~/auth/session";
import { userStateLabel } from "~/format/labels";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { AccessView } from "~/kaeso/access-view";
import { useCatalogue } from "~/kaeso/catalogue";
import { asGrant, sameGrant, type Grant } from "~/kaeso/grant";
import { GrantEditor } from "~/kaeso/rights-editor";
import { holdsAll, rolesOfScope, useRoles } from "~/kaeso/roles";
import { projectPath } from "~/shell/pages";
import { toast } from "~/shell/toast";

const memberQuery = (org: string, user: string) => ({
  queryKey: ["orgs", org, "members", "one", user],
  queryFn: async (): Promise<MemberView> => data(await api.GET("/platform/v1/orgs/{org}/members/{user}", { params: { path: { org, user } } })),
});

export function OrgMemberSheet({ org, userId, onClose }: { org: string; userId: string | undefined; onClose: () => void }) {
  const member = useQuery({ ...memberQuery(org, userId ?? ""), enabled: !!userId });
  return (
    <Sheet open={!!userId} onOpenChange={(o) => !o && onClose()} modal={false} size="md" title={member.data?.user.name ?? t("org.member.title", "Member")}>
      {userId && <MemberBody key={userId} org={org} userId={userId} onClose={onClose} />}
    </Sheet>
  );
}

function MemberBody({ org, userId, onClose }: { org: string; userId: string; onClose: () => void }) {
  const navigate = useNavigate();
  const needs = useNeedsRight(org);
  const session = useSession();
  const rights = useOrgRights(org);
  const membership = useMembership(org);
  const giver = useOrgGiver(org);
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const member = useQuery(memberQuery(org, userId));
  const access = useQuery({ ...memberRightsQuery(org, userId), enabled: rights.has("members.view") });
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const [grant, setGrant] = useState<Grant | null>(null);
  const [removing, setRemoving] = useState(false);
  const [transferring, setTransferring] = useState(false);
  const invalidates = [["orgs", org, "members"], ["orgs", org, "roles"], ["session"]];

  const save = useApiMutation({
    mutationFn: async (g: Grant) => data(await api.PATCH("/platform/v1/orgs/{org}/members/{user}", { params: { path: { org, user: userId } }, body: g })),
    invalidates,
    onSuccess: (m) => {
      setGrant(null);
      toast.success(t("org.member.roleSaved", "{name} is {role} now", { name: m.user.name, role: m.grant.role.name }));
    },
  });
  const remove = useApiMutation({
    mutationFn: async (target: MemberView) => {
      await api.DELETE("/platform/v1/orgs/{org}/members/{user}", { params: { path: { org, user: userId } } });
      return target;
    },
    invalidates: [...invalidates, ["orgs", org, "projects"]],
    onSuccess: (target) => {
      toast(
        target.user.id === session.user.id
          ? t("org.member.left", "You left the organization")
          : t("org.member.removed", "{name} removed from the organization", { name: target.user.name }),
      );
      onClose();
    },
    onError: () => setRemoving(false),
  });
  const transfer = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/ownership-transfer", { params: { path: { org } }, body: { user_id: userId } })),
    invalidates: [...invalidates, ["me", "memberships"]],
    onSuccess: (m) => {
      setTransferring(false);
      toast.success(t("org.ownership.transferred", "{name} is an owner now; you're an admin", { name: m.user.name }));
    },
    onError: () => setTransferring(false),
  });

  if (member.isPending || !cat || !roles || (rights.has("members.view") && access.isPending)) return <Skeleton className="w-full" />;
  if (member.isError) {
    const gone = isApiError(member.error) && member.error.type === "not_found";
    return (
      <Empty
        title={gone ? t("org.member.gone", "This member doesn't exist or was removed.") : t("org.member.unavailable", "This member can't be shown right now.")}
        actions={<Button onClick={onClose}>{t("org.member.back", "Back to members")}</Button>}
      />
    );
  }
  const m = member.data;
  const self = m.user.id === session.user.id;
  const iAmOwner = membership?.role.id === "owner";
  const theyOwn = m.grant.role.id === "owner";
  const owners = (members.data ?? []).filter((x) => x.grant.role.id === "owner").length;
  const lastOwner = theyOwn && owners === 1;
  const peer = iAmOwner || (holdsAll(giver.held, m.grant.rights) && holdsAll(everyProjectRights(session, org), m.grant.project_rights ?? []));
  const why = (right: string): string | undefined =>
    !rights.has(right)
      ? needs(right)
      : theyOwn && !iAmOwner
        ? t("org.member.ownersOnly", "Only owners can change or remove an owner")
        : !peer && !self
          ? t("org.member.notPeer", "{name} holds rights you don't hold, so you can't change them", { name: m.user.name })
          : undefined;
  const lastOwnerReason = t("org.member.lastOwner", "An organization keeps at least one owner; make someone else an owner first");
  const current = asGrant(m.grant);
  const editing = grant ?? current;
  const changed = !!grant && !sameGrant(grant, current);
  const roleReason = why("members.rights.update") ?? (lastOwner ? lastOwnerReason : undefined);
  const removeReason = (self ? undefined : why("members.remove")) ?? (lastOwner ? lastOwnerReason : undefined);
  const canTransfer = iAmOwner && !theyOwn && !self && rights.has("org.ownership.transfer") && m.user.state === "active";
  const projects = access.data?.projects ?? [];

  return (
    <div className="flex flex-col gap-32">
      <KeyValue
        items={[
          { label: common.email(), value: m.user.email ?? common.none(), ...(m.user.email ? { copy: m.user.email } : {}) },
          { label: t("org.member.joined", "Joined"), value: formatDate(m.joined_at) },
          { label: t("org.member.userId", "User id"), value: m.user.id, copy: m.user.id, mono: true },
          ...(m.user.state !== "active" ? [{ label: t("org.member.account", "Account"), value: userStateLabel(m.user.state) }] : []),
        ]}
      />
      <Section title={common.role()} description={t("org.member.roleHint", "What they can do in the organization and in every project.")} spacing="none">
        <div className="flex flex-col gap-12">
          <GrantEditor cat={cat} scope="org" roles={rolesOfScope(roles, "org")} value={editing} onChange={setGrant} giver={giver} readOnly={roleReason} />
          {changed && (
            <div className="flex gap-8">
              <Button size="sm" variant="primary" loading={save.isPending} onClick={() => grant && save.mutate(grant)}>
                {t("org.saveRole", "Save role")}
              </Button>
              <Button size="sm" onClick={() => setGrant(null)}>
                {common.cancel()}
              </Button>
            </div>
          )}
        </div>
      </Section>
      {rights.has("members.view") && (
        <Section
          title={t("org.member.access", "Access")}
          description={t("org.member.accessHint", "Every right they hold, and the role or right it comes from.")}
          spacing="none"
        >
          <div className="flex flex-col gap-24">
            <AccessView
              cat={cat}
              scope="org"
              rights={access.data?.org.rights}
              label={t("org.member.orgRights", "{name}'s rights in the organization", { name: m.user.name })}
            />
            {!!access.data?.every_project.length && (
              <div className="flex flex-col gap-8">
                <Text variant="small" weight="medium">
                  {t("org.inEveryProject", "In every project")}
                </Text>
                <AccessView
                  cat={cat}
                  scope="project"
                  rights={access.data.every_project}
                  label={t("org.member.projectRights", "{name}'s rights in every project", { name: m.user.name })}
                />
              </div>
            )}
          </div>
        </Section>
      )}
      {rights.has("members.view") && (
        <Section title={common.projects()} description={t("org.member.projectsHint", "The projects they're in, with their role in each.")} spacing="none">
          <List
            aria-label={t("org.member.projects", "{name}'s projects", { name: m.user.name })}
            density="compact"
            loading={access.isPending}
            items={projects.map((p) => ({
              id: p.project.id,
              textValue: p.project.name,
              title: p.project.name,
              leading: <Avatar name={p.project.name} {...(p.project.avatar_url ? { src: p.project.avatar_url } : {})} kind="org" size={20} decorative />,
              meta: (
                <>
                  <Badge>{p.role?.name ?? t("org.member.throughOrgRole", "Through their organization role")}</Badge>
                </>
              ),
            }))}
            onOpen={(item) => void navigate({ href: `${projectPath(org, item.id)}/members?member=${userId}` })}
            empty={
              <Empty
                size="compact"
                title={t("org.member.noProjects", "In no projects")}
                description={t("org.member.noProjectsHint", "Add them from a project's Members page.")}
              />
            }
          />
        </Section>
      )}
      {canTransfer && (
        <Section
          title={t("org.ownership.title", "Ownership")}
          description={t("org.member.ownershipHint", "Make them an owner in your place; you become an admin.")}
          spacing="none"
        >
          <div className="flex">
            <Button size="sm" onClick={() => setTransferring(true)}>
              {t("org.ownership.transfer", "Transfer ownership")}
            </Button>
          </div>
        </Section>
      )}
      <Section title={self ? t("org.member.leaveTitle", "Leave the organization") : t("org.member.remove", "Remove from organization")} spacing="none">
        <div className="flex">
          <Button
            variant="danger-secondary"
            size="sm"
            disabled={!!removeReason}
            {...(removeReason ? { disabledReason: removeReason } : {})}
            onClick={() => setRemoving(true)}
          >
            {self ? t("org.member.leave", "Leave organization") : t("org.member.remove", "Remove from organization")}
          </Button>
        </div>
      </Section>
      <AlertDialog
        open={transferring}
        onOpenChange={setTransferring}
        tone="warning"
        title={t("org.member.makeOwner", "Make {name} an owner?", { name: m.user.name })}
        description={t("org.member.makeOwnerHint", "They become an owner with every right, and you become an admin.")}
        confirmLabel={t("org.ownership.transfer", "Transfer ownership")}
        loading={transfer.isPending}
        onConfirm={() => transfer.mutateAsync()}
      />
      <AlertDialog
        open={removing}
        onOpenChange={setRemoving}
        tone="danger"
        title={
          self
            ? t("org.member.leaveConfirm", "Leave the organization?")
            : t("org.member.removeConfirm", "Remove {name} from the organization?", { name: m.user.name })
        }
        description={t("org.member.removeHint", "They leave every project, and their user keys in the organization are archived.")}
        confirmLabel={self ? t("org.member.leave", "Leave organization") : t("org.member.removeMember", "Remove member")}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync(m)}
      />
    </div>
  );
}
