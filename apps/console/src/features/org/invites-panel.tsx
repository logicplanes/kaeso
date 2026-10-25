import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, DialogScope, Empty, FilterRow, IconButton, List, Menu, Text, icons } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { ScopeTabs, type Scope } from "~/kaeso/scope-tabs";
import { api, data } from "~/api/client";
import type { InviteView } from "~/api/generated/types";
import { invitesQuery, orgMembersQuery } from "~/api/queries/orgs";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights } from "~/auth/session";
import { inviteStateLabel } from "~/format/labels";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const STATE_TONE: Record<string, "neutral" | "success" | "warning"> = { pending: "neutral", accepted: "success", expired: "warning", revoked: "neutral" };

function expiresIn(expiresAt: string, now: number): { text: string; soon: boolean } {
  const ms = Date.parse(expiresAt) - now;
  if (ms <= 0) return { text: t("org.invites.expired", "expired"), soon: true };
  const soon = ms < 2 * DAY;
  const days = Math.ceil(ms / DAY);
  if (days >= 2) return { text: t("org.invites.expiresInDays", "{count, plural, one {expires in # day} other {expires in # days}}", { count: days }), soon };
  const hours = Math.ceil(ms / HOUR);
  return { text: t("org.invites.expiresInHours", "{count, plural, one {expires in # hour} other {expires in # hours}}", { count: hours }), soon };
}

export function InvitesPanel({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const [scope, setScope] = useState<Scope>("active");
  const all = scope === "all";
  const invites = useQuery({ ...invitesQuery(org, !all), enabled: rights.has("members.view") });
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const projects = useQuery(orgProjectsQuery(org, true));
  const [revoking, setRevoking] = useState<InviteView | null>(null);
  const [revokeOpen, setRevokeOpen] = useState(false);
  const [now] = useState(Date.now);
  const canInvite = rights.has("members.invite");
  const canRevoke = rights.has("members.invites.revoke");

  const resend = useApiMutation({
    mutationFn: async (i: InviteView) => data(await api.POST("/platform/v1/orgs/{org}/invites/{invite}/resend", { params: { path: { org, invite: i.id } } })),
    invalidates: [["orgs", org, "invites"]],
    onSuccess: (i) => toast(t("org.invites.resent", "Invitation sent to {email} again", { email: i.email })),
  });
  const revoke = (i: InviteView) => {
    setRevoking(i);
    setRevokeOpen(true);
  };

  const invitedBy = (user: string | null | undefined) => {
    if (!user) return t("org.invites.byInstanceAdmin", "Invited by an instance admin");
    const name = members.data?.find((m) => m.user.id === user)?.user.name;
    return name ? t("org.invites.by", "Invited by {name}", { name }) : t("org.invites.byFormerMember", "Invited by a former member");
  };
  const projectName = (id: string) => projects.data?.find((p) => p.id === id)?.name ?? t("org.invites.aProject", "A project");
  const list = [...(invites.data ?? [])].sort((a, b) => b.created_at.localeCompare(a.created_at));
  const items = list.map((i) => {
    const pending = i.status === "pending";
    const expiry = pending ? expiresIn(i.expires_at, now) : null;
    const shown = i.project_roles.slice(0, 1);
    const more = i.project_roles.length - shown.length;
    return {
      id: i.id,
      textValue: i.email,
      title: i.email,
      subtitle: (
        <>
          {invitedBy(i.invited_by)}
          {expiry && (
            <>
              {" · "}
              {expiry.soon ? (
                <Text variant="small" tone="warning">
                  {expiry.text}
                </Text>
              ) : (
                expiry.text
              )}
            </>
          )}
        </>
      ),
      meta: (
        <>
          {!pending && <Badge tone={STATE_TONE[i.status] ?? "neutral"}>{inviteStateLabel(i.status)}</Badge>}
          {i.role && <Badge>{i.role.name}</Badge>}
          {shown.map((r) => (
            <Badge key={r.project_id}>{`${projectName(r.project_id)} · ${r.role.name}`}</Badge>
          ))}
          {more > 0 && <Badge>{t("org.invites.moreProjects", "{count, plural, one {+# project} other {+# projects}}", { count: more })}</Badge>}
        </>
      ),
      date: t("org.invites.sentOn", "Sent {date}", { date: formatDate(i.created_at) }),
      actions:
        pending && (canInvite || canRevoke) ? (
          <Menu>
            <Menu.Trigger>
              <IconButton icon={icons.more} aria-label={t("org.invites.actions", "Actions for the invitation to {email}", { email: i.email })} size="xs" />
            </Menu.Trigger>
            <Menu.Content align="end">
              {canInvite && (
                <Menu.Item icon={icons.mail} onClick={() => resend.mutate(i)}>
                  {t("org.invites.resend", "Resend")}
                </Menu.Item>
              )}
              {canRevoke && (
                <Menu.Item icon={icons.delete} variant="danger" onClick={() => revoke(i)}>
                  {t("org.invites.revoke", "Revoke")}
                </Menu.Item>
              )}
            </Menu.Content>
          </Menu>
        ) : undefined,
    };
  });

  return (
    <>
      <FilterRow
        tabs={
          <ScopeTabs
            label={t("org.invites.title", "Invitations")}
            value={scope}
            onChange={setScope}
            activeLabel={t("org.invites.pending", "Pending")}
            archivedLabel={null}
          />
        }
      />
      <List
        aria-label={t("org.invites.title", "Invitations")}
        items={items}
        wrapSubtitle
        loading={invites.isPending}
        empty={
          invites.isError ? (
            <LoadFailed title={t("org.invites.loadFailed", "The invitations couldn't load")} size="md" />
          ) : all ? (
            <Empty
              size="md"
              title={t("org.invites.noneYet", "No invitations yet")}
              description={t("org.invites.noneYetHint", "Invite people by email; each link works for 21 days.")}
            />
          ) : (
            <Empty
              size="md"
              title={t("org.invites.nonePending", "No pending invitations")}
              description={t("org.invites.nonePendingHint", "Accepted, revoked and expired invitations are under All.")}
            />
          )
        }
      />
      {revoking && (
        <DialogScope open={revokeOpen}>
          <RevokeDialog org={org} invite={revoking} open={revokeOpen} onOpenChange={setRevokeOpen} />
        </DialogScope>
      )}
    </>
  );
}

function RevokeDialog({ org, invite, open, onOpenChange }: { org: string; invite: InviteView; open: boolean; onOpenChange: (open: boolean) => void }) {
  const revoke = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/invites/{invite}", { params: { path: { org, invite: invite.id } } });
    },
    invalidates: [["orgs", org, "invites"]],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("org.invites.revoked", "Invitation revoked"));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("org.invites.revokeConfirm", "Revoke the invitation to {email}?", { email: invite.email })}
      description={t("org.invites.revokeHint", "The link in the email stops working; you can invite the address again later.")}
      confirmLabel={t("org.invites.revokeInvitation", "Revoke invitation")}
      loading={revoke.isPending}
      onConfirm={() => revoke.mutateAsync()}
    />
  );
}
