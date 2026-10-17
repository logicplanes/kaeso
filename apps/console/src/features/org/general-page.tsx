import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Avatar,
  Button,
  Combobox,
  Dialog,
  DialogScope,
  InlineCopyText,
  Input,
  KeyValue,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  Skeleton,
  StepDialog,
  Text,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { MemberView } from "~/api/generated/types";
import { orgMembersQuery, orgQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights, useSession } from "~/auth/session";
import { common } from "~/i18n/common";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import { PictureControl, toPictureUpload } from "~/kaeso/picture";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import { useNeedsRight } from "~/auth/needs-right";

export function OrgGeneralPage({ org }: { org: string }) {
  const session = useSession();
  const rights = useOrgRights(org);
  const view = useQuery(orgQuery(org));
  const o = view.data;
  return (
    <OrgPage org={org} title={t("org.general.title", "General")} hidden={t("org.general.hidden", "You can't see the general settings here")} right="org.view">
      {view.isError ? (
        <LoadFailed title={t("org.general.loadFailed", "The organization couldn't load")} />
      ) : !o ? (
        <Skeleton className="h-full w-full" />
      ) : (
        <>
          <SettingsSection title={common.organization()}>
            <SettingsGroup>
              <PictureRow org={org} name={o.name} src={o.avatar_url} canUpdate={rights.has("org.update")} />
              <NameRow key={o.name} org={org} name={o.name} canUpdate={rights.has("org.update")} />
              <SettingsRow
                title={t("org.general.id", "Organization id")}
                description={t("org.general.idHint", "For the Platform API and support.")}
                control={<InlineCopyText value={o.id} label={t("org.general.copyId", "Copy organization id")} />}
              />
              <SettingsRow title={common.created()} control={<Text tone="secondary">{formatDate(o.created_at)}</Text>} />
            </SettingsGroup>
          </SettingsSection>
          {rights.has("org.ownership.transfer") && <TransferOwnership org={org} />}
          {rights.has("org.delete") && o.state === "active" && session.org_deletion.allowed && <DeleteOrg org={org} name={o.name} />}
        </>
      )}
    </OrgPage>
  );
}

function PictureRow({ org, name, src, canUpdate }: { org: string; name: string; src: string | null | undefined; canUpdate: boolean }) {
  const needs = useNeedsRight(org);
  const path = { params: { path: { org } } };
  const set = useApiMutation({
    mutationFn: async (file: File) => data(await api.PUT("/platform/v1/orgs/{org}/avatar", { ...path, body: await toPictureUpload(file) })),
    invalidates: [["orgs", org], ["session"], ["me", "memberships"]],
    onSuccess: () => toast.success(t("org.general.pictureSaved", "Picture saved")),
  });
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}/avatar", path)),
    invalidates: [["orgs", org], ["session"], ["me", "memberships"]],
    onSuccess: () => toast(t("org.general.pictureRemoved", "Picture removed")),
  });
  return (
    <SettingsRow
      title={t("org.general.picture", "Picture")}
      description={t("org.general.pictureHint", "Shown in the sidebar and to members, cropped to a square.")}
      control={
        <PictureControl
          name={name}
          src={src}
          kind="org"
          onPick={(f) => set.mutate(f)}
          onRemove={() => remove.mutate()}
          busy={set.isPending || remove.isPending}
          disabledReason={canUpdate ? undefined : needs("org.update")}
        />
      }
    />
  );
}

function NameRow({ org, name, canUpdate }: { org: string; name: string; canUpdate: boolean }) {
  const needs = useNeedsRight(org);
  const [draft, setDraft] = useState(name);
  const save = useApiMutation({
    mutationFn: async (next: string) => data(await api.PATCH("/platform/v1/orgs/{org}", { params: { path: { org } }, body: { name: next } })),
    invalidates: [["orgs", org], ["session"], ["me", "memberships"]],
    onSuccess: () => toast.success(t("org.general.nameSaved", "Name saved")),
  });
  const next = draft.trim();
  const reason = !canUpdate
    ? needs("org.update")
    : !next
      ? t("org.general.enterName", "Enter a name")
      : next === name
        ? t("org.noChange", "No change to save")
        : undefined;
  return (
    <SettingsRow
      title={common.name()}
      description={t("org.general.nameHint", "Shown to members and in emails.")}
      control={
        <div className="flex min-w-0 items-center gap-8">
          <div className="w-240 min-w-0">
            <Input
              aria-label={t("org.general.name", "Organization name")}
              size="sm"
              value={draft}
              maxLength={100}
              disabled={!canUpdate}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !reason && !save.isPending && save.mutate(next)}
            />
          </div>
          <Button
            size="sm"
            disabled={!!reason}
            {...(reason && canUpdate ? { disabledReason: reason } : {})}
            loading={save.isPending}
            onClick={() => save.mutate(next)}
          >
            {common.save()}
          </Button>
        </div>
      }
    />
  );
}

function TransferOwnership({ org }: { org: string }) {
  const session = useSession();
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const candidates = (members.data ?? []).filter((m) => m.user.id !== session.user.id && m.user.state === "active");
  const [open, setOpen] = useState(false);
  return (
    <SettingsSection title={t("org.ownership.title", "Ownership")}>
      <SettingsGroup>
        <SettingsRow
          title={t("org.ownership.transfer", "Transfer ownership")}
          description={t("org.ownership.transferHint", "Make another member an owner; you become an admin.")}
          control={
            <Button size="sm" onClick={() => setOpen(true)}>
              {t("org.ownership.transfer", "Transfer ownership")}
            </Button>
          }
        />
      </SettingsGroup>
      <DialogScope open={open}>
        <TransferDialog org={org} candidates={candidates} loading={members.isPending} open={open} onOpenChange={setOpen} />
      </DialogScope>
    </SettingsSection>
  );
}

function TransferDialog({
  org,
  candidates,
  loading,
  open,
  onOpenChange: setOpen,
}: {
  org: string;
  candidates: MemberView[];
  loading: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [user, setUser] = useState<string | null>(null);
  const chosen = candidates.find((m) => m.user.id === user);
  const transfer = useApiMutation({
    mutationFn: async (id: string) => data(await api.POST("/platform/v1/orgs/{org}/ownership-transfer", { params: { path: { org } }, body: { user_id: id } })),
    invalidates: [["orgs", org, "members"], ["session"], ["me", "memberships"]],
    onSuccess: (m) => {
      setOpen(false);
      toast.success(t("org.ownership.transferred", "{name} is an owner now; you're an admin", { name: m.user.name }));
    },
  });
  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={t("org.ownership.transfer", "Transfer ownership")}
      description={
        chosen
          ? t("org.ownership.becomes", "{name} becomes an owner and you become an admin.", { name: chosen.user.name })
          : t("org.ownership.choose", "Choose the member who becomes an owner; you become an admin.")
      }
      onSubmit={() => user && transfer.mutate(user)}
      footer={
        <>
          <Button onClick={() => setOpen(false)}>{common.cancel()}</Button>
          <Button
            variant="primary"
            disabled={!user}
            {...(user ? {} : { disabledReason: t("org.ownership.chooseMember", "Choose a member") })}
            loading={transfer.isPending}
            onClick={() => user && transfer.mutate(user)}
          >
            {t("org.ownership.transfer", "Transfer ownership")}
          </Button>
        </>
      }
    >
      <Combobox
        label={t("org.ownership.newOwner", "New owner")}
        placeholder={loading ? t("org.ownership.loadingMembers", "Loading members…") : t("org.ownership.chooseMember", "Choose a member")}
        options={candidates.map((m) => ({
          value: m.user.id,
          label: m.user.name,
          description: m.user.email ?? undefined,
          leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
        }))}
        value={user}
        onValueChange={(v: string | null) => setUser(v)}
      />
    </Dialog>
  );
}

function DeleteOrg({ org, name }: { org: string; name: string }) {
  const session = useSession();
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState("");
  const grace = t("org.days", "{count, plural, one {# day} other {# days}}", { count: session.org_deletion.grace_days });
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}", { params: { path: { org } } })),
    invalidates: [["orgs", org], ["session"], ["me", "memberships"]],
    onSuccess: (o) => {
      close();
      toast(t("org.delete.scheduled", "{name} is scheduled for deletion", { name: o.name }));
    },
    onError: () => close(),
  });
  const close = () => {
    setOpen(false);
    setTyped("");
  };
  const matches = typed.trim() === name;
  return (
    <SettingsSection title={t("org.delete.title", "Delete organization")}>
      <SettingsGroup>
        <SettingsRow
          danger
          title={t("org.delete.row", "Delete this organization")}
          description={t("org.delete.rowHint", "Keys stop at once; it stays restorable for {grace}.", { grace })}
          control={
            <Button size="sm" variant="danger-secondary" onClick={() => setOpen(true)}>
              {t("org.delete.title", "Delete organization")}
            </Button>
          }
        />
      </SettingsGroup>
      {open && (
        <StepDialog
          open
          onOpenChange={(o) => !o && close()}
          title={t("org.delete.dialog", "Delete {name}", { name })}
          tone="danger"
          submitLabel={t("org.delete.title", "Delete organization")}
          submitting={remove.isPending}
          onSubmit={() => matches && remove.mutate()}
          steps={[
            {
              id: "what",
              title: t("org.delete.what", "What happens"),
              description: t("org.delete.whatHint", "Members are told by email; it's deleted for good after {grace}.", { grace }),
              content: (
                <KeyValue
                  items={[
                    {
                      label: t("org.delete.atOnce", "At once"),
                      value: t("org.delete.atOnceText", "Every API key of the organization stops working."),
                      wrap: true,
                    },
                    {
                      label: t("org.delete.during", "For {grace}", { grace }),
                      value: t("org.delete.duringText", "Members still see everything, read-only; an owner can restore it."),
                      wrap: true,
                    },
                    {
                      label: t("org.delete.then", "Then"),
                      value: t("org.delete.thenText", "Projects, keys, provider keys, settings, usage and logs are removed."),
                      wrap: true,
                    },
                  ]}
                />
              ),
            },
            {
              id: "confirm",
              title: t("org.delete.confirm", "Type its name to confirm"),
              blockedReason: matches ? undefined : t("org.delete.typeName", "Type {name} to confirm", { name }),
              content: (
                <Input
                  aria-label={t("org.general.name", "Organization name")}
                  placeholder={name}
                  value={typed}
                  onChange={(e) => setTyped(e.target.value)}
                  autoComplete="off"
                />
              ),
            },
          ]}
        />
      )}
    </SettingsSection>
  );
}
