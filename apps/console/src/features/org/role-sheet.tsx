import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Avatar, Button, Empty, Input, KeyValue, Link, List, RightsMatrix, Section, Sheet, Skeleton, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { RoleView } from "~/api/generated/types";
import { orgMembersQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useOrgGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { everyProjectRights, useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { rightNames, rightsGroups, useCatalogue, type Catalogue } from "~/kaeso/catalogue";
import { roleSize, useRoles, type Giver } from "~/kaeso/roles";
import { toast } from "~/shell/toast";
import { applyRightChanges } from "./role-dialog";

interface Draft {
  name: string;
  description: string;
  rights: string[];
  project_rights: string[];
}

const same = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x) => b.includes(x));
const heldShort = (role: RoleView) =>
  role.scope === "org"
    ? t("org.role.memberHolders", "{count, plural, one {# member} other {# members}}", { count: role.holders })
    : t("org.role.projectHolders", "{count, plural, one {# member or service account} other {# members and service accounts}}", { count: role.holders });

function cantManage(
  cat: Catalogue | undefined,
  role: RoleView,
  giver: Giver,
  everyProject: ReadonlySet<string>,
  needs: (right: string) => string,
  canManage: boolean,
): string | undefined {
  if (!canManage) return needs("roles.manage");
  if (giver.isOwner) return undefined;
  const mine = role.scope === "org" ? giver.held : everyProject;
  const missing = [...role.rights.filter((r) => !mine.has(r)), ...role.project_rights.filter((r) => !everyProject.has(r))];
  return missing.length ? t("org.role.notHeld", "You can't change it: you don't hold {rights}", { rights: rightNames(cat, [...new Set(missing)]) }) : undefined;
}

export function RoleSheet({
  org,
  roleKey,
  onClose,
  onDuplicate,
}: {
  org: string;
  roleKey: string | undefined;
  onClose: () => void;
  onDuplicate: (role: RoleView) => void;
}) {
  const roles = useRoles(org);
  const [scope, id] = roleKey?.split(":") ?? [];
  const role = roles?.find((r) => r.scope === scope && r.id === id);
  return (
    <Sheet open={!!roleKey} onOpenChange={(o) => !o && onClose()} modal={false} size="lg" title={role?.name ?? common.role()}>
      {roleKey &&
        (!roles ? (
          <Skeleton className="w-full" />
        ) : !role ? (
          <Empty
            title={t("org.role.gone", "This role doesn't exist or was deleted.")}
            actions={<Button onClick={onClose}>{t("org.role.back", "Back to roles")}</Button>}
          />
        ) : (
          <RoleBody key={roleKey} org={org} role={role} onClose={onClose} onDuplicate={onDuplicate} />
        ))}
    </Sheet>
  );
}

function RoleBody({ org, role, onClose, onDuplicate }: { org: string; role: RoleView; onClose: () => void; onDuplicate: (role: RoleView) => void }) {
  const navigate = useNavigate();
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const giver = useOrgGiver(org);
  const session = useSession();
  const cat = useCatalogue(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: role.scope === "org" && rights.has("members.view") });
  const [draft, setDraft] = useState<Draft | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const custom = role.kind === "custom";
  const reason = custom
    ? cantManage(cat, role, giver, everyProjectRights(session, org), needs, rights.has("roles.manage"))
    : t("org.role.builtIn", "Built-in roles can't be changed; duplicate it to start a custom role");
  const editing = draft ?? { name: role.name, description: role.description, rights: role.rights, project_rights: role.project_rights };
  const changed =
    !!draft &&
    (draft.name.trim() !== role.name ||
      draft.description.trim() !== role.description ||
      !same(draft.rights, role.rights) ||
      !same(draft.project_rights, role.project_rights));
  const heldHere = giver.isOwner ? undefined : role.scope === "org" ? giver.held : giver.everyProject;
  const holders = (members.data ?? []).filter((m) => m.grant.role.id === role.id);

  const save = useApiMutation({
    mutationFn: async (d: Draft) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/roles/{role}", {
          params: { path: { org, role: role.id } },
          body: {
            name: d.name.trim(),
            description: d.description.trim(),
            rights: d.rights,
            ...(role.scope === "org" ? { project_rights: d.project_rights } : {}),
          },
        }),
      ),
    invalidates: [["orgs", org, "roles"], ["orgs", org, "members"], ["session"], ["project-rights"]],
    onSuccess: (r) => {
      setDraft(null);
      setConfirming(false);
      toast.success(
        r.holders
          ? t(
              "org.role.savedFor",
              "{count, plural, one {{name} saved for # member or service account} other {{name} saved for # members and service accounts}}",
              {
                name: r.name,
                count: r.holders,
              },
            )
          : t("org.role.saved", "{name} saved", { name: r.name }),
      );
    },
    onError: () => setConfirming(false),
  });
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/roles/{role}", { params: { path: { org, role: role.id } } });
    },
    invalidates: [["orgs", org, "roles"]],
    onSuccess: () => {
      toast(t("org.role.deleted", "{name} deleted", { name: role.name }));
      onClose();
    },
    onError: () => setDeleting(false),
  });

  const edit = (patch: Partial<Draft>) => setDraft({ ...editing, ...patch });
  const deleteReason =
    reason ??
    (role.holders
      ? t(
          "org.role.stillHeld",
          "{count, plural, one {Held by # member or service account; give them another role first} other {Held by # members and service accounts; give them another role first}}",
          { count: role.holders },
        )
      : undefined);
  const owner = role.scope === "org" && role.id === "owner";
  const saveReason = !editing.name.trim() ? t("org.role.nameIt", "Name the role") : undefined;

  return (
    <div className="flex flex-col gap-32">
      <KeyValue
        items={[
          { label: t("org.role.type", "Type"), value: custom ? t("org.roles.custom", "Custom") : t("org.role.builtInType", "Built-in") },
          {
            label: t("org.role.appliesTo", "Applies to"),
            value: role.scope === "org" ? t("org.role.theOrg", "The organization") : t("org.role.aProject", "A project"),
          },
          { label: t("org.role.size", "Size"), value: owner ? t("org.role.everyRightLater", "Every right, now and in later releases") : roleSize(role) },
          {
            label: t("org.role.heldBy", "Held by"),
            value:
              role.scope === "org" && role.holders && rights.has("members.view") ? (
                <Link href={`/settings/orgs/${org}/members?role=${role.id}`}>{heldShort(role)}</Link>
              ) : role.holders ? (
                heldShort(role)
              ) : (
                t("org.roles.nobodyYet", "Nobody yet")
              ),
          },
          ...(role.updated_at ? [{ label: t("org.role.changed", "Changed"), value: formatDate(role.updated_at) }] : []),
        ]}
      />
      {custom && !reason ? (
        <Section title={t("org.role.nameAndDescription", "Name and description")} spacing="none">
          <div className="flex flex-col gap-16">
            <Input label={common.name()} value={editing.name} maxLength={50} onChange={(e) => edit({ name: e.target.value })} />
            <Input
              label={t("org.role.description", "Description")}
              optional
              value={editing.description}
              maxLength={200}
              onChange={(e) => edit({ description: e.target.value })}
            />
          </div>
        </Section>
      ) : (
        role.description && <Text tone="secondary">{role.description}</Text>
      )}
      {cat ? (
        <>
          <Section title={role.scope === "org" ? t("org.role.inOrg", "In the organization") : t("org.role.inProject", "In a project")} spacing="none">
            <RightsMatrix
              searchable
              readOnly={!!reason}
              aria-label={t("org.role.rightsOf", "Rights of {name}", { name: role.name })}
              groups={rightsGroups(cat, role.scope === "org" ? "org" : "project", { granted: new Set(editing.rights), held: heldHere })}
              onRightsChange={(c) => edit({ rights: applyRightChanges(editing.rights, c) })}
            />
          </Section>
          {role.scope === "org" && (
            <Section
              title={t("org.inEveryProject", "In every project")}
              description={
                owner
                  ? t("org.role.ownerEveryProject", "Owners hold every right in every project.")
                  : t("org.role.everyProjectGives", "What it gives in every project of the organization.")
              }
              spacing="none"
            >
              <RightsMatrix
                searchable
                readOnly={!!reason}
                aria-label={t("org.role.rightsOfEveryProject", "Rights of {name} in every project", { name: role.name })}
                groups={rightsGroups(cat, "project", { granted: new Set(editing.project_rights), held: giver.isOwner ? undefined : giver.everyProject })}
                onRightsChange={(c) => edit({ project_rights: applyRightChanges(editing.project_rights, c) })}
              />
            </Section>
          )}
        </>
      ) : (
        <Skeleton className="w-full" />
      )}
      {role.scope === "org" && rights.has("members.view") && (
        <Section title={t("org.role.whoHasIt", "Who has it")} spacing="none">
          <List
            aria-label={t("org.role.membersWith", "Members with {name}", { name: role.name })}
            density="compact"
            loading={members.isPending}
            items={holders.map((m) => ({
              id: m.user.id,
              textValue: m.user.name,
              title: m.user.name,
              subtitle: m.user.email ?? undefined,
              leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
            }))}
            onOpen={(item) => void navigate({ to: "/settings/orgs/$org/members", params: { org }, search: { member: item.id } })}
            empty={
              <Empty
                size="compact"
                title={t("org.role.nobodyHasIt", "No members have it")}
                description={t("org.role.nobodyHasItHint", "Give it from a member's sheet, or when inviting someone.")}
              />
            }
          />
        </Section>
      )}
      <div className="flex flex-wrap gap-8">
        {custom && !reason && (
          <>
            <Button
              size="sm"
              variant="primary"
              disabled={!changed || !!saveReason}
              {...(changed && saveReason ? { disabledReason: saveReason } : {})}
              loading={save.isPending}
              onClick={() => {
                if (role.holders) setConfirming(true);
                else save.mutate(editing);
              }}
            >
              {t("org.saveRole", "Save role")}
            </Button>
            <Button size="sm" disabled={!changed} onClick={() => setDraft(null)}>
              {common.cancel()}
            </Button>
          </>
        )}
        <Button
          size="sm"
          disabled={!rights.has("roles.manage")}
          {...(rights.has("roles.manage") ? {} : { disabledReason: needs("roles.manage") })}
          onClick={() => onDuplicate(role)}
        >
          {t("org.role.duplicate", "Duplicate")}
        </Button>
        {custom && (
          <Button
            variant="danger-secondary"
            size="sm"
            disabled={!!deleteReason}
            {...(deleteReason ? { disabledReason: deleteReason } : {})}
            onClick={() => setDeleting(true)}
          >
            {t("org.role.delete", "Delete role")}
          </Button>
        )}
      </div>
      <AlertDialog
        open={confirming}
        onOpenChange={setConfirming}
        tone="warning"
        title={t(
          "org.role.changeConfirm",
          "{count, plural, one {Change {name} for # member or service account?} other {Change {name} for # members and service accounts?}}",
          { name: role.name, count: role.holders },
        )}
        description={t("org.role.changeHint", "Changes apply at once to everyone with this role.")}
        confirmLabel={t("org.saveRole", "Save role")}
        loading={save.isPending}
        onConfirm={() => save.mutateAsync(editing)}
      />
      <AlertDialog
        open={deleting}
        onOpenChange={setDeleting}
        tone="danger"
        title={t("org.role.deleteConfirm", "Delete {name}?", { name: role.name })}
        description={t("org.role.deleteHint", "Nobody holds it; it leaves every role picker in the organization.")}
        confirmLabel={t("org.role.delete", "Delete role")}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync()}
      />
    </div>
  );
}
