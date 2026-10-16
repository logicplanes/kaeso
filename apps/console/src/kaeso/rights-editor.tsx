import { useState } from "react";
import { Collapsible, RightsMatrix, RolePicker, Skeleton, Text } from "@logicplanes/kaeso-ui";
import type { RoleView } from "~/api/generated/types";
import { t } from "~/i18n";
import { rightsGroups, type Catalogue, type Scope } from "./catalogue";
import { applyChanges, effectiveRights, type Grant } from "./grant";
import { roleOptions, type Giver } from "./roles";

export function GrantEditor({
  cat,
  scope,
  roles,
  value,
  onChange,
  giver,
  readOnly,
  defaultOpen = false,
  labelled = false,
}: {
  cat: Catalogue | undefined;
  scope: Scope;
  roles: RoleView[] | undefined;
  value: Grant;
  onChange: (g: Grant) => void;
  giver: Giver | null;
  readOnly?: string | undefined;
  defaultOpen?: boolean;
  labelled?: boolean;
}) {
  const role = roles?.find((r) => r.id === value.role);
  const changes = value.rights_added.length + value.rights_removed.length;
  const [openAtFirst] = useState(defaultOpen || changes > 0);
  if (!cat || !roles) return <Skeleton className="w-full" />;
  const base = new Set(role?.rights ?? []);
  const granted = effectiveRights(base, value);
  const owner = scope === "org" && value.role === "owner";
  const lockReason = readOnly ?? (owner ? t("kaeso.rights.ownerLocked", "Owners hold every right; give another role to hold fewer") : undefined);
  return (
    <div className="flex flex-col gap-16">
      <div className="flex flex-col gap-6">
        <RolePicker
          appearance="select"
          {...(labelled ? { label: t("common.role", "Role") } : { "aria-label": t("common.role", "Role") })}
          fullWidth
          disabled={!!readOnly}
          roles={roleOptions(cat, roles, giver)}
          value={value.role}
          onValueChange={(next) => onChange({ role: next, rights_added: [], rights_removed: [] })}
        />
        {role?.description && (
          <Text variant="small" tone="secondary">
            {role.description}
          </Text>
        )}
      </div>
      <Collapsible
        title={t("kaeso.rights.customize", "Customize")}
        hint={
          changes
            ? t("kaeso.rights.countWithChanges", "{rights, plural, one {# right} other {# rights}} · {changes, plural, one {# change} other {# changes}}", {
                rights: granted.size,
                changes,
              })
            : t("kaeso.rights.count", "{count, plural, one {# right} other {# rights}}", { count: granted.size })
        }
        defaultOpen={openAtFirst}
      >
        <RightsMatrix
          searchable
          aria-label={scope === "org" ? t("kaeso.rights.inOrg", "Rights in the organization") : t("kaeso.rights.inProject", "Rights in the project")}
          groups={rightsGroups(cat, scope, { granted, base, held: giver?.isOwner ? undefined : giver?.held, readOnly: lockReason })}
          onRightsChange={(c) => !lockReason && onChange(applyChanges(base, value, c, cat.order[scope]))}
        />
      </Collapsible>
    </div>
  );
}
