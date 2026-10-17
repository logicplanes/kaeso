import { useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AlertDialog,
  Avatar,
  Badge,
  Button,
  DataTable,
  Dialog,
  Empty,
  FilterRow,
  Input,
  KeyValue,
  SearchInput,
  Section,
  PillTabs,
  SettingsGroup,
  SettingsRow,
  Sheet,
  SkeletonLine,
  StatusDot,
  Switch,
  Text,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { InstanceUserView } from "~/api/generated/types";
import { instanceUserQuery, instanceUsersQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { userStateLabel } from "~/format/labels";
import { usePageCreateAction } from "~/shell/create-action";
import { toast } from "~/shell/toast";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";
import { patchSearch, type SearchPatch } from "~/search";

const STATES = ["active", "deactivated", "deleted"] as const;
type UserState = (typeof STATES)[number];

export type UsersSearch = { q?: string; state?: UserState; user?: string; invite?: boolean };

const STATE_DOT: Record<string, "warning" | "neutral"> = { deactivated: "warning", deleted: "neutral" };

export function InstanceUsersPage({ search }: { search: UsersSearch }) {
  const navigate = useNavigate();
  const [q, setQ] = useState(search.q ?? "");
  const users = useInfiniteQuery(instanceUsersQuery(search.q ?? "", search.state));
  const [inviting, setInviting] = useState(!!search.invite);
  usePageCreateAction({ label: t("instance.inviteUser", "Invite user"), run: () => setInviting(true) });
  const setSearch = (patch: SearchPatch<UsersSearch>) => {
    void navigate({ to: "/settings/instance/users", search: patchSearch(search, patch, ["invite"]), replace: true });
  };
  const rows = users.data?.pages.flatMap((p) => p.data) ?? [];

  const columns = [
    col.custom(
      { id: "name", header: t("common.name", "Name"), size: 232, priority: 1, grow: true },
      (u: InstanceUserView) => u.name,
      (_v, u) => (
        <span className="flex min-w-0 items-center gap-8">
          <Avatar name={u.name} {...(u.avatar_url ? { src: u.avatar_url } : {})} size={20} decorative />
          <span className="truncate">{u.name}</span>
          {u.is_instance_admin && <Badge>{t("instance.admin", "Admin")}</Badge>}
        </span>
      ),
    ),
    col.text({ id: "email", header: t("common.email", "Email"), size: 192, priority: 2 }, (u: InstanceUserView) => u.email ?? ""),
    col.custom(
      { id: "state", header: t("instance.state", "State"), size: 104, priority: 1 },
      (u: InstanceUserView) => u.state,
      (v: string) => (STATE_DOT[v] ? <StatusDot tone={STATE_DOT[v]}>{userStateLabel(v)}</StatusDot> : <Text>{userStateLabel(v)}</Text>),
    ),
    col.custom(
      { id: "created", header: t("common.created", "Created"), size: 136, priority: 3 },
      (u: InstanceUserView) => u.created_at,
      (v: string) => <time dateTime={v}>{formatDate(v)}</time>,
    ),
  ];

  return (
    <InstancePage
      title={t("instance.users", "Users")}
      description={t("instance.usersHint", "Everyone with an account on this install.")}
      actions={
        <Button size="sm" variant="primary" leadingIcon={icons.add} onClick={() => setInviting(true)}>
          {t("instance.inviteUser", "Invite user")}
        </Button>
      }
    >
      <FilterRow
        tabs={
          <PillTabs value={search.state ?? "all"} onValueChange={(v) => setSearch({ state: STATES.find((s) => s === v), user: undefined })}>
            <PillTabs.List aria-label={t("instance.state", "State")}>
              <PillTabs.Tab value="all">{t("common.all", "All")}</PillTabs.Tab>
              {STATES.map((id) => (
                <PillTabs.Tab key={id} value={id}>
                  {userStateLabel(id)}
                </PillTabs.Tab>
              ))}
            </PillTabs.List>
          </PillTabs>
        }
      >
        <div className="w-240">
          <SearchInput
            aria-label={t("instance.searchUsers", "Search users")}
            placeholder={t("instance.nameOrEmail", "Name or email")}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onClear={() => {
              setQ("");
              setSearch({ q: undefined });
            }}
            onKeyDown={(e) => e.key === "Enter" && setSearch({ q: q.trim() || undefined, user: undefined })}
            onBlur={() => q.trim() !== (search.q ?? "") && setSearch({ q: q.trim() || undefined })}
          />
        </div>
      </FilterRow>
      <DataTable
        aria-label={t("instance.users", "Users")}
        data={rows}
        columns={columns}
        getRowId={(u) => u.id}
        loading={users.isPending}
        error={users.isError && rows.length === 0 ? t("instance.usersFailed", "Users couldn't load") : undefined}
        onRetry={() => void users.refetch()}
        empty={
          <Empty
            size="md"
            title={search.q ? t("instance.noUsersMatch", "No users match") : search.state ? noUsersIn(search.state) : t("instance.noUsers", "No users")}
            {...(search.q ? { description: t("instance.searchAnotherNameOrEmail", "Search by another name or email address.") } : {})}
          />
        }
        hasMore={users.hasNextPage}
        loadingMore={users.isFetchingNextPage}
        onLoadMore={() => void users.fetchNextPage()}
        onRowClick={(u) => setSearch({ user: u.id })}
        activeRowId={search.user}
      />
      <UserSheet userId={search.user} onClose={() => setSearch({ user: undefined })} />
      {inviting && (
        <InviteUserDialog
          onClose={() => {
            setInviting(false);
            if (search.invite) setSearch({});
          }}
        />
      )}
    </InstancePage>
  );
}

function noUsersIn(state: UserState): string {
  if (state === "active") return t("instance.noActiveUsers", "No active users");
  if (state === "deactivated") return t("instance.noDeactivatedUsers", "No deactivated users");
  return t("instance.noDeletedUsers", "No deleted users");
}

function InviteUserDialog({ onClose }: { onClose: () => void }) {
  const [email, setEmail] = useState("");
  const invite = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/users/invites", { body: { email: email.trim() } })),
    invalidates: [["instance", "users"]],
    onSuccess: (i) => {
      toast.success(t("instance.invitationSent", "Invitation sent to {email}", { email: i.email }));
      onClose();
    },
  });
  const reason = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim()) ? undefined : t("instance.enterEmail", "Enter an email address");
  return (
    <Dialog
      busy={invite.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("instance.inviteUser", "Invite user")}
      description={t("instance.inviteUserHint", "They get a link to create an account, and join no organization until one invites them.")}
      onSubmit={() => !reason && invite.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            disabled={!!reason}
            {...(reason ? { disabledReason: reason } : {})}
            loading={invite.isPending}
            onClick={() => invite.mutate()}
          >
            {t("instance.sendInvitation", "Send invitation")}
          </Button>
        </>
      }
    >
      <Input label={t("common.email", "Email")} type="email" autoComplete="off" value={email} onChange={(e) => setEmail(e.target.value)} autoFocus />
    </Dialog>
  );
}

function UserSheet({ userId, onClose }: { userId: string | undefined; onClose: () => void }) {
  const user = useQuery({ ...instanceUserQuery(userId ?? ""), enabled: !!userId });
  return (
    <Sheet open={!!userId} onOpenChange={(o) => !o && onClose()} modal={false} size="md" title={user.data?.name ?? t("instance.user", "User")}>
      {userId && <UserBody key={userId} userId={userId} onClose={onClose} />}
    </Sheet>
  );
}

type Confirm = "deactivate" | "sessions" | "reset-2fa" | "delete" | null;

function UserBody({ userId, onClose }: { userId: string; onClose: () => void }) {
  const session = useSession();
  const user = useQuery(instanceUserQuery(userId));
  const [confirm, setConfirm] = useState<Confirm>(null);
  const invalidates = [["instance", "users"]];
  const path = { params: { path: { user: userId } } };

  const deactivate = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/users/{user}/deactivate", path)),
    invalidates,
    onSuccess: (u) => {
      setConfirm(null);
      toast(t("instance.userDeactivated", "{name} deactivated", { name: u.name }));
    },
    onError: () => setConfirm(null),
  });
  const reactivate = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/users/{user}/reactivate", path)),
    invalidates,
    onSuccess: (u) => toast.success(t("instance.userReactivated", "{name} reactivated", { name: u.name })),
  });
  const endSessions = useApiMutation({
    mutationFn: async () => {
      await api.POST("/platform/v1/instance/users/{user}/sessions/revoke", path);
    },
    onSuccess: () => {
      setConfirm(null);
      toast(t("instance.sessionsEnded", "Every session ended"));
    },
    onError: () => setConfirm(null),
  });
  const reset2fa = useApiMutation({
    mutationFn: async () => {
      await api.POST("/platform/v1/instance/users/{user}/reset-2fa", path);
    },
    invalidates,
    onSuccess: () => {
      setConfirm(null);
      toast(t("instance.twoFactorReset", "Two-factor authentication reset"));
    },
    onError: () => setConfirm(null),
  });
  const admin = useApiMutation({
    mutationFn: async (on: boolean) => data(await api.PUT("/platform/v1/instance/users/{user}/instance-admin", { ...path, body: { is_instance_admin: on } })),
    invalidates: [...invalidates, ["session"]],
    onSuccess: (u) =>
      toast.success(
        u.is_instance_admin
          ? t("instance.nowAdmin", "{name} is an instance admin", { name: u.name })
          : t("instance.noLongerAdmin", "{name} is no longer an instance admin", { name: u.name }),
      ),
  });
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/instance/users/{user}", path);
    },
    invalidates,
    onSuccess: () => {
      toast(t("instance.userDeleted", "User deleted"));
      onClose();
    },
    onError: () => setConfirm(null),
  });

  if (user.isPending) return <SkeletonLine lines={5} />;
  if (user.isError) {
    const gone = isApiError(user.error) && user.error.type === "not_found";
    return (
      <Empty
        title={gone ? t("instance.userGone", "This user doesn't exist.") : t("instance.userUnavailable", "This user can't be shown right now.")}
        actions={<Button onClick={onClose}>{t("instance.backToUsers", "Back to users")}</Button>}
      />
    );
  }
  const u = user.data;
  const self = u.id === session.user.id;
  const deleted = u.state === "deleted";
  return (
    <div className="flex flex-col gap-24">
      <KeyValue
        items={[
          { label: t("common.email", "Email"), value: u.email ?? t("common.none", "None"), ...(u.email ? { copy: u.email } : {}) },
          { label: t("instance.state", "State"), value: userStateLabel(u.state) },
          { label: t("instance.twoFactor", "Two-factor"), value: u.totp_enabled ? t("common.on", "On") : t("common.off", "Off") },
          { label: t("common.created", "Created"), value: formatDate(u.created_at) },
          { label: t("instance.userId", "User id"), value: u.id, copy: u.id, mono: true },
        ]}
      />
      {!deleted && (
        <Section title={t("instance.access", "Access")} spacing="none">
          <SettingsGroup>
            <SettingsRow
              title={t("instance.instanceAdmin", "Instance admin")}
              description={t("instance.instanceAdminHint", "Manages the install; holds no rights inside organizations.")}
              control={
                <Switch
                  aria-label={t("instance.instanceAdmin", "Instance admin")}
                  checked={u.is_instance_admin}
                  loading={admin.isPending}
                  onCheckedChange={(on) => !admin.isPending && admin.mutate(on)}
                />
              }
            />
            {u.state === "active" ? (
              <SettingsRow
                title={t("instance.deactivate", "Deactivate")}
                description={t("instance.deactivateHint", "Blocks sign-in and disables their keys until reactivated.")}
                control={
                  <Button
                    size="sm"
                    disabled={self}
                    {...(self ? { disabledReason: t("instance.cantDeactivateSelf", "You can't deactivate yourself") } : {})}
                    onClick={() => setConfirm("deactivate")}
                  >
                    {t("instance.deactivate", "Deactivate")}
                  </Button>
                }
              />
            ) : (
              <SettingsRow
                title={t("instance.reactivate", "Reactivate")}
                description={t("instance.reactivateHint", "They can sign in again and their keys work again.")}
                control={
                  <Button size="sm" variant="primary" loading={reactivate.isPending} onClick={() => reactivate.mutate()}>
                    {t("instance.reactivate", "Reactivate")}
                  </Button>
                }
              />
            )}
            <SettingsRow
              title={t("instance.endSessions", "End sessions")}
              description={t("instance.endSessionsHint", "Signs them out everywhere.")}
              control={
                <Button size="sm" onClick={() => setConfirm("sessions")}>
                  {t("instance.endSessions", "End sessions")}
                </Button>
              }
            />
            {u.totp_enabled && (
              <SettingsRow
                title={t("instance.reset2fa", "Reset two-factor")}
                description={t("instance.reset2faHint", "Removes their authenticator app and recovery codes.")}
                control={
                  <Button size="sm" onClick={() => setConfirm("reset-2fa")}>
                    {t("instance.reset2fa", "Reset two-factor")}
                  </Button>
                }
              />
            )}
          </SettingsGroup>
        </Section>
      )}
      {!deleted && (
        <Section title={t("instance.deleteUser", "Delete user")} spacing="none">
          <div className="flex flex-col items-start gap-8">
            <Text variant="small" tone="secondary">
              {t("instance.deleteUserHint", 'Their name and email are removed; they appear as "Deleted user" in history.')}
            </Text>
            <Button
              size="sm"
              variant="danger-secondary"
              disabled={self}
              {...(self ? { disabledReason: t("instance.deleteSelfInProfile", "Delete your own account in your profile") } : {})}
              onClick={() => setConfirm("delete")}
            >
              {t("instance.deleteUser", "Delete user")}
            </Button>
          </div>
        </Section>
      )}
      <AlertDialog
        open={confirm === "deactivate"}
        onOpenChange={(o) => !o && setConfirm(null)}
        tone="danger"
        title={t("instance.deactivateQuestion", "Deactivate {name}?", { name: u.name })}
        description={t("instance.deactivateConfirmHint", "They can't sign in, every session ends and their keys are disabled until you reactivate them.")}
        confirmLabel={t("instance.deactivate", "Deactivate")}
        loading={deactivate.isPending}
        onConfirm={() => deactivate.mutate()}
      />
      <AlertDialog
        open={confirm === "sessions"}
        onOpenChange={(o) => !o && setConfirm(null)}
        tone="warning"
        title={t("instance.endSessionsQuestion", "End {name}'s sessions?", { name: u.name })}
        description={t("instance.endSessionsConfirmHint", "They're signed out everywhere and get an email about it.")}
        confirmLabel={t("instance.endSessions", "End sessions")}
        loading={endSessions.isPending}
        onConfirm={() => endSessions.mutate()}
      />
      <AlertDialog
        open={confirm === "reset-2fa"}
        onOpenChange={(o) => !o && setConfirm(null)}
        tone="warning"
        title={t("instance.reset2faQuestion", "Reset {name}'s two-factor authentication?", { name: u.name })}
        description={t("instance.reset2faConfirmHint", "Their authenticator app and recovery codes stop working; they get an email about it.")}
        confirmLabel={t("instance.reset2fa", "Reset two-factor")}
        loading={reset2fa.isPending}
        onConfirm={() => reset2fa.mutate()}
      />
      <AlertDialog
        open={confirm === "delete"}
        onOpenChange={(o) => !o && setConfirm(null)}
        tone="danger"
        title={t("instance.deleteUserQuestion", "Delete {name}?", { name: u.name })}
        description={t("instance.deleteUserConfirmHint", "Their name and email are removed and their keys archived; this can't be undone.")}
        confirmLabel={t("instance.deleteUser", "Delete user")}
        confirmText={u.email ?? u.name}
        loading={remove.isPending}
        onConfirm={() => remove.mutate()}
      />
    </div>
  );
}
