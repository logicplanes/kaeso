import { useState } from "react";
import { keepPreviousData, useInfiniteQuery, useQuery } from "@tanstack/react-query";
import {
  Avatar,
  Badge,
  Button,
  Combobox,
  Dialog,
  DialogScope,
  Empty,
  InlineCopyText,
  Input,
  KeyValue,
  List,
  Pill,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  Skeleton,
  SkeletonLine,
  StatGrid,
  StatTile,
  StepDialog,
  Text,
} from "@logicplanes/kaeso-ui";
import { formatBytes, formatDate, formatNumber, formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { InstanceOrgUsage, InstanceOrgView } from "~/api/generated/types";
import { instanceOrgMembersQuery, instanceOrgProjectsQuery, instanceOrgQuery, instanceOrgUsageQuery, instanceUsersQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { userStateLabel } from "~/format/labels";
import { grantLabel } from "~/kaeso/grant";
import { toast } from "~/shell/toast";
import { InstancePage } from "./instance-page";
import { DeleteNowDialog, useRestoreOrg } from "./org-deletion";
import { t } from "~/i18n";

const TOUCHED = [["instance", "orgs"], ["session"], ["me", "memberships"]];

export function InstanceOrgPage({ org }: { org: string }) {
  const view = useQuery(instanceOrgQuery(org));
  const session = useSession();
  const member = session.memberships.some((m) => m.org_id === org);
  const o = view.data;
  if (view.isError) {
    const gone = isApiError(view.error) && view.error.type === "not_found";
    return (
      <InstancePage title={t("common.organization", "Organization")}>
        <Empty
          title={
            gone
              ? t("instance.orgGone", "This organization doesn't exist or was removed.")
              : t("instance.orgUnavailable", "This organization can't be shown right now.")
          }
          actions={<Button href="/settings/instance/orgs">{t("instance.backToOrgs", "Back to organizations")}</Button>}
        />
      </InstancePage>
    );
  }
  return (
    <InstancePage
      title={o?.name ?? t("common.organization", "Organization")}
      description={t("instance.orgPageHint", "You see its metadata, not its content; changes go in its audit log.")}
      {...(member && o?.state === "active" ? { actions: <Button href={`/orgs/${org}`}>{t("instance.openOrg", "Open organization")}</Button> } : {})}
    >
      <Usage org={org} />
      {!o ? (
        <SettingsSection title={t("common.organization", "Organization")}>
          <SkeletonLine lines={4} />
        </SettingsSection>
      ) : (
        <>
          <Facts org={org} o={o} />
          <Members org={org} count={o.members} />
          <Projects org={org} count={o.projects} />
        </>
      )}
    </InstancePage>
  );
}

function Usage({ org }: { org: string }) {
  const usage = useQuery(instanceOrgUsageQuery(org));
  const u = usage.data;
  return (
    <SettingsSection
      title={t("instance.last30Days", "Last 30 days")}
      description={t("instance.last30DaysHint", "Requests, tokens and cost across its projects, and the logs it stores")}
    >
      <StatGrid aria-label={t("instance.usage30Days", "Usage in the last 30 days")}>{u ? <UsageTiles u={u} /> : <LoadingTiles />}</StatGrid>
    </SettingsSection>
  );
}

function UsageTiles({ u }: { u: InstanceOrgUsage }) {
  const n = u.requests_30d;
  const perRequest = (v: number) => v / n;
  return (
    <>
      <StatTile
        label={t("common.requests", "Requests")}
        value={formatNumber(n)}
        detail={
          n ? t("instance.perDay", "{count} a day on average", { count: formatNumber(Math.round(n / 30)) }) : t("instance.noneInPeriod", "None in this period")
        }
      />
      <StatTile
        label={t("common.tokens", "Tokens")}
        value={formatTokens(u.tokens_30d)}
        detail={n ? t("instance.perRequest", "{value} per request", { value: formatTokens(Math.round(perRequest(u.tokens_30d))) }) : undefined}
      />
      <StatTile
        label={t("common.cost", "Cost")}
        value={formatUSD(u.cost_30d)}
        detail={
          n ? t("instance.perRequest", "{value} per request", { value: formatUSD(perRequest(Number(u.cost_30d)).toFixed(8), { significant: 2 }) }) : undefined
        }
      />
      <StatTile
        label={t("instance.logs", "Logs")}
        value={formatBytes(u.log_logical_bytes)}
        detail={t("instance.entryCount", "{count, plural, one {# entry} other {# entries}}", { count: u.log_entries })}
      />
    </>
  );
}

function LoadingTiles() {
  return (
    <>
      {[t("common.requests", "Requests"), t("common.tokens", "Tokens"), t("common.cost", "Cost"), t("instance.logs", "Logs")].map((label) => (
        <StatTile key={label} label={label} value={<Skeleton shape="line" className="w-64" />} />
      ))}
    </>
  );
}

function signIn(o: InstanceOrgView): string {
  if (o.require_sso && o.require_2fa) return t("instance.ssoAnd2fa", "Single sign-on and two-factor");
  if (o.require_sso) return t("instance.sso", "Single sign-on");
  if (o.require_2fa) return t("instance.twoFactorAuth", "Two-factor authentication");
  return t("common.none", "None");
}

function Facts({ org, o }: { org: string; o: InstanceOrgView }) {
  return (
    <SettingsSection title={t("common.organization", "Organization")}>
      <SettingsGroup>
        <SettingsRow title={t("common.name", "Name")} description={o.name} control={<Rename o={o} />} />
        <SettingsRow
          title={t("instance.orgId", "Organization id")}
          control={<InlineCopyText value={o.id} label={t("instance.copyOrgId", "Copy organization id")} />}
        />
        <SettingsRow title={t("common.created", "Created")} control={<Text tone="secondary">{formatDate(o.created_at)}</Text>} />
        <SettingsRow
          title={t("instance.signIn", "Sign-in")}
          description={t("instance.signInHint", "What the organization requires of its members.")}
          control={<Text tone="secondary">{signIn(o)}</Text>}
        />
        <SettingsRow
          title={t("instance.requestLogs", "Request logs")}
          description={
            o.content_logging
              ? t("instance.keptWithContent", "Metadata kept {metadata} days, content {content} days.", {
                  metadata: o.metadata_retention_days,
                  content: o.content_retention_days,
                })
              : t("instance.keptMetadata", "Metadata kept {metadata} days.", { metadata: o.metadata_retention_days })
          }
          control={<Text tone="secondary">{o.content_logging ? t("instance.withContent", "With content") : t("instance.metadataOnly", "Metadata only")}</Text>}
        />
        <SettingsRow
          title={t("instance.owners", "Owners")}
          description={
            o.owners === 0
              ? t("instance.noOwners", "Nobody can manage it until an owner is appointed.")
              : t("instance.ownerCount", "{count, plural, one {# owner} other {# owners}}", { count: o.owners })
          }
          control={o.state === "active" ? <AppointOwner org={org} hasOwner={o.owners > 0} /> : undefined}
        />
        {o.state === "scheduled_for_deletion" ? <ScheduledRows o={o} /> : <ScheduleDeletionRow o={o} />}
      </SettingsGroup>
    </SettingsSection>
  );
}

function Members({ org, count }: { org: string; count: number }) {
  const members = useQuery(instanceOrgMembersQuery(org));
  const list = members.data ?? [];
  const description =
    count > list.length && list.length
      ? t("instance.firstMembers", "The first {shown} of {count, plural, one {# member} other {# members}}, with their role and the day they joined", {
          shown: formatNumber(list.length),
          count,
        })
      : t("instance.memberCount", "{count, plural, one {# member} other {# members}}, with their role and the day they joined", { count });
  return (
    <SettingsSection title={t("common.members", "Members")} description={description}>
      {!members.isPending && list.length === 0 ? (
        <Text tone="secondary">{t("instance.noMembers", "No members.")}</Text>
      ) : (
        <List
          aria-label={t("common.members", "Members")}
          loading={members.isPending}
          items={list.map((m) => ({
            id: m.user.id,
            textValue: m.user.name,
            title: m.user.name,
            subtitle: m.user.email ?? undefined,
            leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
            meta: (
              <>
                {m.user.state !== "active" && <Badge tone="warning">{userStateLabel(m.user.state)}</Badge>}
                <Pill size="sm">{grantLabel(m.grant)}</Pill>
              </>
            ),
            date: formatDate(m.joined_at),
          }))}
        />
      )}
    </SettingsSection>
  );
}

function Projects({ org, count }: { org: string; count: number }) {
  const projects = useQuery(instanceOrgProjectsQuery(org));
  const list = projects.data ?? [];
  const description =
    count > list.length && list.length
      ? t("instance.firstProjects", "The first {shown} of {count, plural, one {# project} other {# projects}}, with the day each was created", {
          shown: formatNumber(list.length),
          count,
        })
      : t("instance.projectCount", "{count, plural, one {# project} other {# projects}}, with the day each was created", { count });
  return (
    <SettingsSection title={t("common.projects", "Projects")} description={description}>
      {!projects.isPending && list.length === 0 ? (
        <Text tone="secondary">{t("instance.noProjects", "No projects.")}</Text>
      ) : (
        <List
          aria-label={t("common.projects", "Projects")}
          loading={projects.isPending}
          items={list.map((p) => ({
            id: p.id,
            textValue: p.name,
            title: p.name,
            leading: <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />,
            meta: p.is_default ? (
              <Badge>{t("instance.default", "Default")}</Badge>
            ) : p.archived_at ? (
              <Badge>{t("common.archived", "Archived")}</Badge>
            ) : undefined,
            date: formatDate(p.created_at),
          }))}
        />
      )}
    </SettingsSection>
  );
}

function AppointOwner({ org, hasOwner }: { org: string; hasOwner: boolean }) {
  const [open, setOpen] = useState(false);
  const members = useQuery(instanceOrgMembersQuery(org));
  const owners = new Set((members.data ?? []).filter((m) => m.grant.role.id === "owner").map((m) => m.user.id));
  return (
    <>
      <Button size="sm" variant={hasOwner ? "secondary" : "primary"} onClick={() => setOpen(true)}>
        {t("instance.addOwner", "Add owner")}
      </Button>
      <DialogScope open={open}>
        <AppointOwnerDialog org={org} owners={owners} open={open} onOpenChange={setOpen} />
      </DialogScope>
    </>
  );
}

function AppointOwnerDialog({
  org,
  owners,
  open,
  onOpenChange: setOpen,
}: {
  org: string;
  owners: Set<string>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const session = useSession();
  const [user, setUser] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const users = useInfiniteQuery({ ...instanceUsersQuery(query, "active"), enabled: open, placeholderData: keepPreviousData });
  const me = session.user.id;
  const appoint = useApiMutation({
    mutationFn: async (id: string) => data(await api.POST("/platform/v1/instance/orgs/{org}/owner", { params: { path: { org } }, body: { user_id: id } })),
    invalidates: TOUCHED,
    onSuccess: (_o, id) => {
      setOpen(false);
      toast.success(id === me ? t("instance.youreOwner", "You're an owner now") : t("instance.ownerAdded", "Owner added"));
    },
  });
  return (
    <Dialog
      busy={appoint.isPending}
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={t("instance.addOwnerTitle", "Add an owner")}
      description={t("instance.addOwnerHint", "They manage it with every right; it's recorded in its audit log.")}
      onSubmit={() => user && appoint.mutate(user)}
      footer={
        <>
          <Button onClick={() => setOpen(false)}>{t("common.cancel", "Cancel")}</Button>
          <Button
            variant="primary"
            disabled={!user}
            {...(user ? {} : { disabledReason: t("instance.chooseUser", "Choose a user") })}
            loading={appoint.isPending && appoint.variables === user}
            onClick={() => user && appoint.mutate(user)}
          >
            {t("instance.addOwnerSubmit", "Add owner")}
          </Button>
        </>
      }
    >
      <Combobox
        label={t("instance.user", "User")}
        placeholder={t("instance.searchUsers", "Search users")}
        options={(users.data?.pages.flatMap((p) => p.data) ?? [])
          .filter((u) => !owners.has(u.id))
          .map((u) => ({
            value: u.id,
            label: u.name,
            description: u.email ?? undefined,
            leading: <Avatar name={u.name} {...(u.avatar_url ? { src: u.avatar_url } : {})} size={20} decorative />,
          }))}
        loading={users.isFetching}
        onInputValueChange={setQuery}
        value={user}
        onValueChange={(v: string | null) => setUser(v)}
        emptyText={t("instance.noUsersMatch", "No users match")}
      />
      {!owners.has(me) && (
        <Button size="sm" className="mt-12 self-start" loading={appoint.isPending && appoint.variables === me} onClick={() => appoint.mutate(me)}>
          {t("instance.addMe", "Add me")}
        </Button>
      )}
    </Dialog>
  );
}

function ScheduledRows({ o }: { o: InstanceOrgView }) {
  const session = useSession();
  const restore = useRestoreOrg();
  const [deleting, setDeleting] = useState(false);
  return (
    <>
      <SettingsRow
        danger
        title={
          o.deletion_due_at
            ? t("instance.deletionOn", "Deletion on {date}", { date: formatDate(o.deletion_due_at) })
            : t("instance.scheduledForDeletion", "Scheduled for deletion")
        }
        description={t("instance.scheduledHint", "Read-only until then; restoring makes its keys work again.")}
        control={
          <Button size="sm" variant="primary" loading={restore.isPending} onClick={() => restore.mutate(o.id)}>
            {t("instance.restoreOrg", "Restore organization")}
          </Button>
        }
      />
      {session.org_deletion.allowed && (
        <SettingsRow
          danger
          title={t("instance.deleteNow", "Delete now")}
          description={t("instance.deleteNowHint", "Removes it for good instead of on its date.")}
          control={
            <Button size="sm" variant="danger-secondary" onClick={() => setDeleting(true)}>
              {t("instance.deleteNow", "Delete now")}
            </Button>
          }
        />
      )}
      {deleting && <DeleteNowDialog org={o} onClose={() => setDeleting(false)} />}
    </>
  );
}

function Rename({ o }: { o: InstanceOrgView }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(o.name);
  const trimmed = name.trim();
  const rename = useApiMutation({
    mutationFn: async () => data(await api.PATCH("/platform/v1/instance/orgs/{org}", { params: { path: { org: o.id } }, body: { name: trimmed } })),
    invalidates: TOUCHED,
    onSuccess: (r) => {
      setOpen(false);
      toast.success(t("instance.orgRenamed", "Renamed to {name}", { name: r.name }));
    },
  });
  const unchanged = trimmed === o.name;
  const reason = !trimmed
    ? t("instance.enterName", "Enter a name")
    : trimmed.length > 100
      ? t("instance.nameTooLong", "Use at most 100 characters")
      : unchanged
        ? t("instance.enterNewName", "Enter a new name")
        : undefined;
  return (
    <>
      <Button size="sm" onClick={() => (setName(o.name), setOpen(true))}>
        {t("common.rename", "Rename")}
      </Button>
      <Dialog
        busy={rename.isPending}
        open={open}
        onOpenChange={setOpen}
        size="sm"
        title={t("instance.renameOrg", "Rename the organization")}
        description={t("instance.renameOrgHint", "Its members see the new name at once.")}
        onSubmit={() => !reason && rename.mutate()}
        footer={
          <>
            <Button onClick={() => setOpen(false)}>{t("common.cancel", "Cancel")}</Button>
            <Button
              variant="primary"
              disabled={!!reason}
              {...(reason ? { disabledReason: reason } : {})}
              loading={rename.isPending}
              onClick={() => rename.mutate()}
            >
              {t("common.rename", "Rename")}
            </Button>
          </>
        }
      >
        <Input aria-label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" autoFocus />
      </Dialog>
    </>
  );
}

function ScheduleDeletionRow({ o }: { o: InstanceOrgView }) {
  const session = useSession();
  const [open, setOpen] = useState(false);
  if (!session.org_deletion.allowed) return null;
  const description = t("instance.deleteOrgHint", "Read-only for {count, plural, one {# day} other {# days}}, then deleted for good.", {
    count: session.org_deletion.grace_days,
  });
  return (
    <>
      <SettingsRow
        danger
        title={t("instance.deleteOrg", "Delete organization")}
        description={description}
        control={
          <Button size="sm" variant="danger-secondary" onClick={() => setOpen(true)}>
            {t("instance.deleteOrg", "Delete organization")}
          </Button>
        }
      />
      {open && <ScheduleDeletionDialog o={o} days={session.org_deletion.grace_days} onClose={() => setOpen(false)} />}
    </>
  );
}

function ScheduleDeletionDialog({ o, days, onClose }: { o: InstanceOrgView; days: number; onClose: () => void }) {
  const [typed, setTyped] = useState("");
  const schedule = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/orgs/{org}/schedule-deletion", { params: { path: { org: o.id } } })),
    invalidates: TOUCHED,
    onSuccess: (r) => {
      toast(
        r.deletion_due_at
          ? t("instance.orgWillBeDeleted", "{name} will be deleted on {date}", { name: r.name, date: formatDate(r.deletion_due_at) })
          : t("instance.orgScheduled", "{name} is scheduled for deletion", { name: r.name }),
      );
      onClose();
    },
    onError: () => onClose(),
  });
  const matches = typed.trim() === o.name;
  return (
    <StepDialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={t("instance.deleteNamed", "Delete {name}", { name: o.name })}
      tone="danger"
      submitLabel={t("instance.deleteOrg", "Delete organization")}
      submitting={schedule.isPending}
      onSubmit={() => matches && schedule.mutate()}
      steps={[
        {
          id: "what",
          title: t("instance.whatHappens", "What happens"),
          description: t(
            "instance.whatHappensHint",
            "Its keys stop at once; it stays read-only for {count, plural, one {# day} other {# days}}, then goes for good.",
            {
              count: days,
            },
          ),
          content: (
            <KeyValue
              items={[
                { label: t("common.organization", "Organization"), value: o.name },
                { label: t("common.members", "Members"), value: formatNumber(o.members) },
                { label: t("common.projects", "Projects"), value: formatNumber(o.projects) },
              ]}
            />
          ),
        },
        {
          id: "confirm",
          title: t("instance.typeNameToConfirm", "Type its name to confirm"),
          description: t("instance.scheduleConfirmHint", "Its members and the instance admins are told by email."),
          blockedReason: matches ? undefined : t("instance.typeNameBlocked", "Type {name} to confirm", { name: o.name }),
          content: (
            <Input
              aria-label={t("instance.orgName", "Organization name")}
              placeholder={o.name}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
            />
          ),
        },
      ]}
    />
  );
}
