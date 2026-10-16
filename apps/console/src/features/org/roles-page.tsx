import { Badge, Empty, List, Section, Skeleton } from "@logicplanes/kaeso-ui";
import type { RoleView } from "~/api/generated/types";
import { t } from "~/i18n";
import { roleSize, rolesOfScope, useRoles } from "~/kaeso/roles";
import { RoleDialog, draftFrom, type RoleDraft } from "./role-dialog";
import { RoleSheet } from "./role-sheet";

const held = (count: number) => (count ? t("org.roles.heldBy", "Held by {count}", { count }) : t("org.roles.nobodyYet", "Nobody yet"));

export function RolesPanel({
  org,
  role,
  onRole,
  draft,
  onDraft,
}: {
  org: string;
  role: string | undefined;
  onRole: (role: string | undefined) => void;
  draft: RoleDraft | null;
  onDraft: (draft: RoleDraft | null) => void;
}) {
  const roles = useRoles(org);
  const open = onRole;
  const rows = (list: RoleView[]) =>
    list.map((r) => ({
      id: `${r.scope}:${r.id}`,
      textValue: r.name,
      title: r.name,
      subtitle: r.description,
      meta: (
        <>
          {r.kind === "custom" && <Badge>{t("org.roles.custom", "Custom")}</Badge>}
          <Badge tone="outline">{r.scope === "org" && r.id === "owner" ? t("org.roles.everyRight", "Every right") : roleSize(r)}</Badge>
        </>
      ),
      date: held(r.holders),
    }));

  return (
    <>
      {!roles ? (
        <Skeleton className="w-full" />
      ) : (
        <div className="mt-16 flex flex-col gap-40">
          <Section
            title={t("org.roles.orgRoles", "Organization roles")}
            description={t("org.roles.orgRolesHint", "One per member: their rights in the organization, and in every project.")}
            spacing="none"
          >
            <List aria-label={t("org.roles.orgRoles", "Organization roles")} items={rows(rolesOfScope(roles, "org"))} onOpen={(item) => open(item.id)} />
          </Section>
          <Section
            title={t("org.roles.projectRoles", "Project roles")}
            description={t("org.roles.projectRolesHint", "One per member or service account in each project they're in.")}
            spacing="none"
          >
            <List
              aria-label={t("org.roles.projectRoles", "Project roles")}
              items={rows(rolesOfScope(roles, "project"))}
              onOpen={(item) => open(item.id)}
              empty={<Empty size="compact" title={t("org.roles.noProjectRoles", "No project roles")} />}
            />
          </Section>
        </div>
      )}
      <RoleSheet
        org={org}
        roleKey={role}
        onClose={() => open(undefined)}
        onDuplicate={(r) => {
          open(undefined);
          onDraft(draftFrom(r));
        }}
      />
      {draft && (
        <RoleDialog
          org={org}
          draft={draft}
          onClose={() => onDraft(null)}
          onCreated={(r) => {
            onDraft(null);
            open(`${r.scope}:${r.id}`);
          }}
        />
      )}
    </>
  );
}
