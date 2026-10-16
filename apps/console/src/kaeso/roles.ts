import { useQuery } from "@tanstack/react-query";
import type { RoleOption } from "@logicplanes/kaeso-ui";
import type { RoleView } from "~/api/generated/types";
import { rolesQuery } from "~/api/queries/rights";
import { t } from "~/i18n";
import { rightNames, type Catalogue, type Scope } from "./catalogue";

export function useRoles(org: string): RoleView[] | undefined {
  return useQuery(rolesQuery(org)).data;
}

export const rolesOfScope = (roles: readonly RoleView[] | undefined, scope: Scope): RoleView[] => (roles ?? []).filter((r) => r.scope === scope);

export interface Giver {
  held: ReadonlySet<string>;
  everyProject?: ReadonlySet<string> | undefined;
  isOwner: boolean;
}

export function cantGive(cat: Catalogue | undefined, role: RoleView, giver: Giver): string | undefined {
  if (role.scope === "org" && role.id === "owner") return giver.isOwner ? undefined : t("kaeso.roles.onlyOwners", "Only owners can make someone an owner");
  if (giver.isOwner) return undefined;
  const missing = [
    ...role.rights.filter((r) => !giver.held.has(r)),
    ...(role.scope === "org" ? role.project_rights.filter((r) => !(giver.everyProject ?? new Set()).has(r)) : []),
  ];
  return missing.length
    ? t("kaeso.roles.cantGive", "You can't give it: you don't hold {rights}", { rights: rightNames(cat, [...new Set(missing)]) })
    : undefined;
}

export function roleSize(role: RoleView): string {
  return role.scope === "org" && role.project_rights.length
    ? t("kaeso.roles.sizeWithProjects", "{rights, plural, one {# right} other {# rights}}, {projectRights} in projects", {
        rights: role.rights.length,
        projectRights: role.project_rights.length,
      })
    : t("kaeso.rights.count", "{count, plural, one {# right} other {# rights}}", { count: role.rights.length });
}

export function roleOptions(cat: Catalogue | undefined, roles: readonly RoleView[], giver: Giver | null, opts: { hint?: boolean } = {}): RoleOption[] {
  return roles.map((r) => {
    const reason = giver ? cantGive(cat, r, giver) : undefined;
    return {
      value: r.id,
      label: r.name,
      description: r.description,
      custom: r.kind === "custom",
      ...(opts.hint ? { hint: roleSize(r) } : {}),
      ...(reason ? { disabled: true, disabledReason: reason } : {}),
    };
  });
}

export function holdsAll(mine: ReadonlySet<string>, theirs: readonly string[]): boolean {
  return theirs.every((r) => mine.has(r));
}
