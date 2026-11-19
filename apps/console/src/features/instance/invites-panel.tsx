import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Button, DialogScope, Empty, FilterRow, List, Menu, Text, icons } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { InviteView } from "~/api/generated/types";
import { instanceInvitesQuery, instanceUserQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { STATE_TONE, expiresIn } from "~/features/org/invites-panel";
import { inviteStateLabel } from "~/format/labels";
import { t } from "~/i18n";
import { ScopeTabs, type Scope } from "~/kaeso/scope-tabs";
import { toast } from "~/shell/toast";

const INVITES = [["instance", "users", "invites"]];

/** The invitations to the install, which join no organization: pending ones, or every one under All. */
export function InstanceInvitesPanel() {
  const [scope, setScope] = useState<Scope>("active");
  const all = scope === "all";
  const invites = useQuery(instanceInvitesQuery(!all));
  const [revoking, setRevoking] = useState<InviteView | null>(null);
  const [revokeOpen, setRevokeOpen] = useState(false);
  const [now] = useState(Date.now);
  const resend = useApiMutation({
    mutationFn: async (i: InviteView) => data(await api.POST("/platform/v1/instance/users/invites/{invite}/resend", { params: { path: { invite: i.id } } })),
    invalidates: INVITES,
    onSuccess: (i) => toast(t("org.invites.resent", "Invitation sent to {email} again", { email: i.email })),
  });
  const list = [...(invites.data ?? [])].sort((a, b) => b.created_at.localeCompare(a.created_at));
  const items = list.map((i) => {
    const pending = i.status === "pending";
    const expiry = pending ? expiresIn(i.expires_at, now) : null;
    return {
      id: i.id,
      textValue: i.email,
      title: i.email,
      subtitle: (
        <>
          <InvitedBy user={i.invited_by} />
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
      meta: !pending ? <Badge tone={STATE_TONE[i.status] ?? "neutral"}>{inviteStateLabel(i.status)}</Badge> : undefined,
      date: t("org.invites.sentOn", "Sent {date}", { date: formatDate(i.created_at) }),
      actions: pending ? (
        <Menu>
          <Menu.Trigger>
            <Button
              variant="ghost"
              icon={icons.more}
              aria-label={t("org.invites.actions", "Actions for the invitation to {email}", { email: i.email })}
              size="xs"
            />
          </Menu.Trigger>
          <Menu.Content align="end">
            <Menu.Item icon={icons.mail} onClick={() => resend.mutate(i)}>
              {t("org.invites.resend", "Resend")}
            </Menu.Item>
            <Menu.Item
              icon={icons.delete}
              variant="danger"
              onClick={() => {
                setRevoking(i);
                setRevokeOpen(true);
              }}
            >
              {t("org.invites.revoke", "Revoke")}
            </Menu.Item>
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
              description={t("instance.invites.noneYetHint", "Invite people to the install with Invite user; each link works for 21 days.")}
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
          <RevokeDialog invite={revoking} open={revokeOpen} onOpenChange={setRevokeOpen} />
        </DialogScope>
      )}
    </>
  );
}

function InvitedBy({ user }: { user: string | null | undefined }) {
  const inviter = useQuery({ ...instanceUserQuery(user ?? ""), enabled: !!user });
  if (inviter.data) return <>{t("org.invites.by", "Invited by {name}", { name: inviter.data.name })}</>;
  return <>{t("org.invites.byInstanceAdmin", "Invited by an instance admin")}</>;
}

function RevokeDialog({ invite, open, onOpenChange }: { invite: InviteView; open: boolean; onOpenChange: (open: boolean) => void }) {
  const revoke = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/instance/users/invites/{invite}", { params: { path: { invite: invite.id } } });
    },
    invalidates: INVITES,
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
