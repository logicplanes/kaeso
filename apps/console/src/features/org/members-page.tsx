import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Avatar, Badge, Button, Empty, FilterRow, List, SearchInput, Select, SwapText, Tabs, icons } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { orgMembersQuery } from "~/api/queries/orgs";
import { useMembership, useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { compareNames } from "~/format/collate";
import { common } from "~/i18n/common";
import { userStateLabel } from "~/format/labels";
import { grantLabel } from "~/kaeso/grant";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { InviteDialog } from "./invite-dialog";
import { InvitesPanel } from "./invites-panel";
import { emptyDraft, type RoleDraft } from "./role-dialog";
import { RolesPanel } from "./roles-page";
import { OrgMemberSheet } from "./member-sheet";
import { usePageCreateAction } from "~/shell/create-action";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import { useNeedsRight } from "~/auth/needs-right";
import { patchSearch, type SearchPatch } from "~/search";

type OrgMembersTab = "members" | "invites" | "roles";
export type OrgMembersSearch = { tab?: "invites" | "roles"; member?: string; role?: string; invite?: boolean; create?: boolean };

export function OrgMembersPage({ org, search }: { org: string; search: OrgMembersSearch }) {
  const needs = useNeedsRight(org);
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const [inviting, setInviting] = useState(!!search.invite);
  const [draft, setDraft] = useState<RoleDraft | null>(search.create ? emptyDraft : null);
  const tab: OrgMembersTab = search.tab ?? "members";
  const roles = tab === "roles";
  const right = roles ? "roles.manage" : "members.invite";
  const can = rights.has(right);
  const reason = can ? undefined : needs(right);
  const run = () => (roles ? setDraft(emptyDraft) : setInviting(true));
  const createRole = t("org.roles.create", "Create role");
  const inviteMember = t("org.members.invite", "Invite members");
  usePageCreateAction({ label: roles ? createRole : inviteMember, run, disabledReason: reason });

  const setSearch = (patch: SearchPatch<OrgMembersSearch>) => {
    void navigate({ to: "/settings/orgs/$org/members", params: { org }, search: patchSearch(search, patch, ["invite", "create"]), replace: true });
  };

  return (
    <OrgPage
      org={org}
      title={common.members()}
      hidden={t("org.members.hidden", "You can't see the members here")}
      right="members.view"
      description={t("org.members.description", "Who is in the organization, who is invited, and what each role can do.")}
      actions={
        <Button size="sm" variant="primary" leadingIcon={icons.add} disabled={!can} {...(reason ? { disabledReason: reason } : {})} onClick={run}>
          <SwapText reserve={[inviteMember, createRole]}>{roles ? createRole : inviteMember}</SwapText>
        </Button>
      }
    >
      <Tabs
        value={tab}
        onValueChange={(next) =>
          void navigate({
            to: "/settings/orgs/$org/members",
            params: { org },
            search: next === "invites" || next === "roles" ? { tab: next } : {},
            replace: true,
          })
        }
      >
        <Tabs.List aria-label={t("org.members.people", "People")}>
          <Tabs.Tab value="members">{common.members()}</Tabs.Tab>
          <Tabs.Tab value="invites">{t("org.members.invites", "Invites")}</Tabs.Tab>
          <Tabs.Tab value="roles">{t("org.members.roles", "Roles")}</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="members">
          <MembersPanel org={org} role={search.role} onRole={(role) => setSearch({ role })} onOpen={(member) => setSearch({ member })} />
        </Tabs.Panel>
        <Tabs.Panel value="invites">
          <InvitesPanel org={org} />
        </Tabs.Panel>
        <Tabs.Panel value="roles">
          <RolesPanel
            org={org}
            role={roles ? search.role : undefined}
            onRole={(role) => setSearch({ role })}
            draft={draft}
            onDraft={(next) => {
              setDraft(next);
              if (!next && search.create) setSearch({});
            }}
          />
        </Tabs.Panel>
      </Tabs>
      <OrgMemberSheet org={org} userId={search.member} onClose={() => setSearch({ member: undefined })} />
      {inviting && (
        <InviteDialog
          org={org}
          onClose={() => {
            setInviting(false);
            if (search.invite) setSearch({});
          }}
        />
      )}
    </OrgPage>
  );
}

function MembersPanel({
  org,
  role,
  onRole,
  onOpen,
}: {
  org: string;
  role: string | undefined;
  onRole: (role: string | undefined) => void;
  onOpen: (member: string) => void;
}) {
  const session = useSession();
  const withSso = !!useMembership(org)?.sso;
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const roles = rolesOfScope(useRoles(org), "org");
  const [query, setQuery] = useState("");
  const words = query.trim().toLocaleLowerCase();
  const list = useMemo(
    () =>
      [...(members.data ?? [])]
        .filter((m) => !role || m.grant.role.id === role)
        .filter((m) => !words || m.user.name.toLocaleLowerCase().includes(words) || !!m.user.email?.toLocaleLowerCase().includes(words))
        .sort((a, b) => compareNames(a.user.name, b.user.name)),
    [members.data, role, words],
  );
  const items = list.map((m) => ({
    id: m.user.id,
    textValue: m.user.name,
    title: m.user.id === session.user.id ? t("org.nameYou", "{name} (you)", { name: m.user.name }) : m.user.name,
    subtitle: m.user.email ?? undefined,
    leading: <Avatar name={m.user.name} {...(m.user.avatar_url ? { src: m.user.avatar_url } : {})} size={20} decorative />,
    meta: (
      <>
        {m.user.state !== "active" && <Badge tone={m.user.state === "deleted" ? "neutral" : "warning"}>{userStateLabel(m.user.state)}</Badge>}
        {withSso && !m.managed && <Badge>{t("org.members.guest", "Guest")}</Badge>}
        <Badge>{grantLabel(m.grant)}</Badge>
      </>
    ),
    date: t("org.members.joined", "Joined {date}", { date: formatDate(m.joined_at) }),
  }));

  return (
    <>
      <FilterRow>
        <Select
          aria-label={common.role()}
          size="sm"
          value={role ?? "all"}
          onValueChange={(v) => onRole(v === "all" ? undefined : v)}
          options={[
            { value: "all", label: t("org.members.everyRole", "Every role") },
            ...roles.map((r) => ({
              value: r.id,
              label: r.name,
              textValue: r.name,
              ...(roles.some((x) => x.kind === "custom")
                ? { group: r.kind === "custom" ? t("org.members.customRoles", "Custom roles") : t("org.members.builtInRoles", "Built-in roles") }
                : {}),
            })),
            ...(role && !roles.some((r) => r.id === role) ? [{ value: role, label: role }] : []),
          ]}
        />
        <div className="w-240">
          <SearchInput
            aria-label={t("org.members.search", "Search members")}
            placeholder={t("org.members.nameOrEmail", "Name or email")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onClear={() => setQuery("")}
          />
        </div>
      </FilterRow>
      <List
        aria-label={common.members()}
        items={items}
        loading={members.isPending}
        onOpen={(item) => onOpen(item.id)}
        empty={
          members.isError ? (
            <LoadFailed title={t("org.members.loadFailed", "The members couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={
                words
                  ? t("org.members.noneMatch", "No members match")
                  : role
                    ? t("org.members.noneWithRole", "No members with the {role} role", { role: roles.find((r) => r.id === role)?.name ?? role })
                    : t("org.members.none", "No members")
              }
              {...(words
                ? { description: t("org.members.searchAnother", "Search by another name or email address.") }
                : role
                  ? { description: t("org.members.noneWithRoleHint", "Choose another role, or every role, to see the others.") }
                  : {})}
            />
          )
        }
      />
    </>
  );
}
