import { useState } from "react";
import { Input, Radio, RadioCards, RightsMatrix, Select, StepDialog, type RightChange } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { RoleView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useOrgGiver } from "~/auth/givers";
import { everyProjectRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { rightsGroups, useCatalogue, type Scope } from "~/kaeso/catalogue";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { toast } from "~/shell/toast";

export interface RoleDraft {
  scope: Scope;
  name: string;
  description: string;
  rights: string[];
  project_rights: string[];
}

export const emptyDraft: RoleDraft = { scope: "project", name: "", description: "", rights: [], project_rights: [] };

export const draftFrom = (r: RoleView): RoleDraft => ({
  scope: r.scope === "org" ? "org" : "project",
  name: t("org.role.copyName", "{name} copy", { name: r.name }).slice(0, 50),
  description: r.description,
  rights: r.rights,
  project_rights: r.project_rights,
});

export const applyRightChanges = (set: readonly string[], changes: readonly RightChange[]) => {
  const out = new Set(set);
  for (const c of changes) {
    if (c.granted) out.add(c.code);
    else out.delete(c.code);
  }
  return [...out];
};

export function RoleDialog({ org, draft, onClose, onCreated }: { org: string; draft: RoleDraft; onClose: () => void; onCreated: (role: RoleView) => void }) {
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const giver = useOrgGiver(org);
  const session = useSession();
  const [role, setRole] = useState(draft);
  const [start, setStart] = useState("none");
  const name = role.name.trim();
  const taken = (roles ?? []).some((r) => r.scope === role.scope && r.name.toLowerCase() === name.toLowerCase());
  const held = giver.isOwner ? undefined : giver.held;
  const heldInProjects = giver.isOwner ? undefined : everyProjectRights(session, org);

  const create = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/roles", {
          params: { path: { org } },
          body: {
            scope: role.scope,
            name,
            description: role.description.trim(),
            rights: role.rights,
            ...(role.scope === "org" ? { project_rights: role.project_rights } : {}),
          },
        }),
      ),
    invalidates: [["orgs", org, "roles"]],
    onSuccess: (r) => {
      toast.success(t("org.role.created", "{name} created", { name: r.name }));
      onCreated(r);
    },
  });

  const nameReason = !name
    ? t("org.role.nameIt", "Name the role")
    : taken
      ? role.scope === "org"
        ? t("org.role.takenOrg", "There is an organization role named {name} already", { name })
        : t("org.role.takenProject", "There is a project role named {name} already", { name })
      : undefined;
  const steps = [
    {
      id: "name",
      title: t("org.role.nameStep", "What is the role called?"),
      description: t("org.role.nameStepHint", "Members see its name and description wherever a role is chosen."),
      blockedReason: nameReason,
      content: (
        <div className="flex flex-col gap-16">
          <Input
            label={common.name()}
            placeholder={t("org.role.namePlaceholder", "Support lead")}
            value={role.name}
            maxLength={50}
            onChange={(e) => setRole({ ...role, name: e.target.value })}
            autoFocus
          />
          <Input
            label={t("org.role.description", "Description")}
            optional
            placeholder={t("org.role.descriptionPlaceholder", "Reads logs and usage to help customers")}
            value={role.description}
            maxLength={200}
            onChange={(e) => setRole({ ...role, description: e.target.value })}
          />
          <RadioCards
            aria-label={t("org.role.scope", "Where it applies")}
            value={role.scope}
            onValueChange={(scope) => {
              setRole({ ...role, scope: scope === "org" ? "org" : "project", rights: [], project_rights: [] });
              setStart("none");
            }}
          >
            <Radio
              value="project"
              label={t("org.role.projectRole", "Project role")}
              description={t("org.role.projectRoleHint", "Given to members and service accounts in a project.")}
            />
            <Radio
              value="org"
              label={t("org.role.orgRole", "Organization role")}
              description={t("org.role.orgRoleHint", "Given to members of the organization; may reach every project.")}
            />
          </RadioCards>
        </div>
      ),
    },
    {
      id: "rights",
      title:
        role.scope === "org" ? t("org.role.rightsOrg", "What can it do in the organization?") : t("org.role.rightsProject", "What can it do in a project?"),
      description: t("org.role.rightsHint", "You can give only rights you hold; what a right needs comes with it."),
      blockedReason: role.rights.length ? undefined : t("org.role.oneRight", "Give the role at least one right"),
      content: cat && (
        <div className="flex flex-col gap-20">
          <Select
            label={t("org.role.startFrom", "Start from")}
            size="sm"
            value={start}
            onValueChange={(id) => {
              setStart(id);
              const from = rolesOfScope(roles, role.scope).find((r) => r.id === id);
              const allowed = role.scope === "org" ? held : heldInProjects;
              setRole({ ...role, rights: (from?.rights ?? []).filter((r) => !allowed || allowed.has(r)) });
            }}
            options={[
              { value: "none", label: t("org.role.noRights", "No rights") },
              ...rolesOfScope(roles, role.scope).map((r) => ({ value: r.id, label: r.name, description: r.description })),
            ]}
          />
          <RightsMatrix
            searchable
            aria-label={t("org.role.rights", "Rights of the role")}
            groups={rightsGroups(cat, role.scope, { granted: new Set(role.rights), held: role.scope === "org" ? held : heldInProjects })}
            onRightsChange={(c) => setRole({ ...role, rights: applyRightChanges(role.rights, c) })}
          />
        </div>
      ),
    },
    ...(role.scope === "org"
      ? [
          {
            id: "projects",
            title: t("org.role.everyProject", "And in every project?"),
            description: t("org.role.everyProjectHint", "Members with this role get these rights in every project, now and new ones; none is fine."),
            content: cat && (
              <RightsMatrix
                searchable
                aria-label={t("org.role.rightsEveryProject", "Rights of the role in every project")}
                groups={rightsGroups(cat, "project", { granted: new Set(role.project_rights), held: heldInProjects })}
                onRightsChange={(c) => setRole({ ...role, project_rights: applyRightChanges(role.project_rights, c) })}
              />
            ),
          },
        ]
      : []),
  ];

  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      size="lg"
      title={t("org.roles.create", "Create role")}
      submitLabel={t("org.roles.create", "Create role")}
      submitting={create.isPending}
      onSubmit={() => !nameReason && create.mutate()}
      steps={steps}
    />
  );
}
