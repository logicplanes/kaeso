import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Button, Empty, KeyValue, Section, Sheet, Skeleton } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { ProjectMemberView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useMembership, useSession } from "~/auth/session";
import { userStateLabel } from "~/format/labels";
import { projectMemberRightsQuery } from "~/api/queries/rights";
import { AccessView } from "~/kaeso/access-view";
import { useCatalogue } from "~/kaeso/catalogue";
import { asGrant, sameGrant, type Grant } from "~/kaeso/grant";
import { LimitsSection } from "~/kaeso/limits-section";
import { ModelAccessSection } from "~/kaeso/model-access-section";
import { GrantEditor } from "~/kaeso/rights-editor";
import { holdsAll, rolesOfScope, useRoles } from "~/kaeso/roles";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { useProjectGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const memberQuery = (org: string, project: string, user: string) => ({
  queryKey: ["orgs", org, "projects", project, "members", "one", user],
  queryFn: async (): Promise<ProjectMemberView> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/members/{user}", { params: { path: { org, project, user } } })),
});

export function MemberSheet({ org, project, userId, onClose }: { org: string; project: string; userId: string | undefined; onClose: () => void }) {
  const member = useQuery({ ...memberQuery(org, project, userId ?? ""), enabled: !!userId });
  return (
    <Sheet
      open={!!userId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={member.data?.user.name ?? t("project.members.member", "Member")}
    >
      {userId && <MemberBody key={userId} org={org} project={project} userId={userId} onClose={onClose} />}
    </Sheet>
  );
}

function MemberBody({ org, project, userId, onClose }: { org: string; project: string; userId: string; onClose: () => void }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const session = useSession();
  const rights = useProjectRights(org, project);
  const giver = useProjectGiver(org, project);
  const membership = useMembership(org);
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const member = useQuery(memberQuery(org, project, userId));
  const access = useQuery(projectMemberRightsQuery(org, project, userId));
  useRememberRecent(
    member.data ? { kind: "member", id: userId, label: member.data.user.name, href: `/orgs/${org}/projects/${project}/members?member=${userId}` } : null,
  );
  const [grant, setGrant] = useState<Grant | null>(null);
  const [removing, setRemoving] = useState(false);
  const invalidates = [["orgs", org, "projects", project, "members"]];

  const save = useApiMutation({
    mutationFn: async (g: Grant) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/members/{user}", { params: { path: { org, project, user: userId } }, body: g })),
    invalidates: [...invalidates, ["project-rights"]],
    onSuccess: (m) => {
      setGrant(null);
      toast.success(t("project.members.roleSaved", "{name} is {role} in this project now", { name: m.user.name, role: m.grant.role.name }));
    },
  });
  const self = userId === session.user.id;
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/members/{user}", { params: { path: { org, project, user: userId } } });
    },
    invalidates: [...invalidates, ["orgs", org, "projects", project, "keys"], ...(self ? [["me", "memberships"], ["project-rights"]] : [])],
    onSuccess: () => {
      if (self) {
        toast(t("project.members.left", "You left the project"));
        void navigate({ to: "/orgs/$org", params: { org } });
        return;
      }
      toast(t("project.members.removed", "{name} removed from the project", { name: member.data?.user.name ?? userId }));
      onClose();
    },
  });

  if (member.isPending || access.isPending || !cat || !roles) return <Skeleton className="w-full" />;
  if (member.isError) {
    return (
      <Empty
        title={
          isApiError(member.error) && member.error.type === "not_found"
            ? t("project.members.notFound", "This member doesn't exist or was removed.")
            : t("project.members.loadFailed", "Couldn't load this member.")
        }
        actions={<Button onClick={onClose}>{t("project.members.back", "Back to members")}</Button>}
      />
    );
  }
  const m = member.data;
  const peer = membership?.role.id === "owner" || holdsAll(rights, m.grant.rights);
  const why = (right: string): string | undefined =>
    !rights.has(right)
      ? needs(right)
      : !peer && !self
        ? t("project.members.peerRule", "{name} holds rights here you don't hold, so you can't change them", { name: m.user.name })
        : undefined;
  const current = asGrant(m.grant);
  const editing = grant ?? current;
  const changed = !!grant && !sameGrant(grant, current);
  const readOnly = why("project.members.rights.update");
  const removeReason = why("project.members.remove");

  return (
    <div className="flex flex-col gap-32">
      <KeyValue
        items={[
          { label: common.email(), value: m.user.email ?? common.none(), ...(m.user.email ? { copy: m.user.email } : {}) },
          { label: t("project.members.joined", "Joined"), value: formatDate(m.joined_at) },
          ...(m.user.state !== "active" ? [{ label: t("project.members.account", "Account"), value: userStateLabel(m.user.state) }] : []),
        ]}
      />
      <Section title={common.role()} description={t("project.members.roleHint", "What they can do in this project.")} spacing="none">
        <div className="flex flex-col gap-12">
          <GrantEditor cat={cat} scope="project" roles={rolesOfScope(roles, "project")} value={editing} onChange={setGrant} giver={giver} readOnly={readOnly} />
          <div className="flex gap-8">
            <Button size="sm" variant="primary" disabled={!changed} loading={save.isPending} onClick={() => grant && save.mutate(grant)}>
              {t("project.members.saveRole", "Save role")}
            </Button>
            <Button size="sm" disabled={!changed} onClick={() => setGrant(null)}>
              {common.cancel()}
            </Button>
          </div>
        </div>
      </Section>
      <Section
        title={t("project.members.access", "Access")}
        description={t("project.members.accessHint", "Every right they hold here, and the role or right it comes from.")}
        spacing="none"
      >
        <AccessView
          cat={cat}
          scope="project"
          rights={access.data?.rights}
          label={t("project.members.rightsLabel", "{name}'s rights in this project", { name: m.user.name })}
        />
      </Section>
      {rights.has("limits.view") && (
        <ModelAccessSection org={org} project={project} subject={{ kind: "user", id: m.user.id }} canEdit={rights.has("limits.manage")} />
      )}
      {rights.has("limits.view") && (
        <LimitsSection
          org={org}
          project={project}
          filter={{ user: m.user.id }}
          target={{ level: "member", user: m.user.id }}
          canEdit={rights.has("limits.manage")}
          empty={t("project.members.noLimits", "No limits on this member; the project's limits apply.")}
        />
      )}
      <Section title={self ? t("project.members.leaveThis", "Leave this project") : t("project.members.remove", "Remove from project")} spacing="none">
        <div className="flex">
          <Button
            variant="danger-secondary"
            size="sm"
            disabled={!!removeReason}
            {...(removeReason ? { disabledReason: removeReason } : {})}
            onClick={() => setRemoving(true)}
          >
            {self ? t("project.members.leave", "Leave project") : t("project.members.remove", "Remove from project")}
          </Button>
        </div>
      </Section>
      <AlertDialog
        open={removing}
        onOpenChange={setRemoving}
        tone="danger"
        title={
          self
            ? t("project.members.leaveTitle", "Leave this project?")
            : t("project.members.removeTitle", "Remove {name} from the project?", { name: m.user.name })
        }
        description={t("project.members.removeHint", "Their keys in this project are archived and stop authenticating; they stay in the organization.")}
        confirmLabel={self ? t("project.members.leave", "Leave project") : t("project.members.remove", "Remove from project")}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync()}
      />
    </div>
  );
}
