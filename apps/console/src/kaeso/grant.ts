import type { RightChange } from "@logicplanes/kaeso-ui";
import type { GrantView } from "~/api/generated/types";
import { t } from "~/i18n";

export interface Grant {
  role: string;
  rights_added: string[];
  rights_removed: string[];
}

export const asGrant = (g: GrantView): Grant => ({ role: g.role.id, rights_added: g.rights_added, rights_removed: g.rights_removed });

export function sameGrant(a: Grant, b: Grant): boolean {
  const same = (x: string[], y: string[]) => x.length === y.length && x.every((r) => y.includes(r));
  return a.role === b.role && same(a.rights_added, b.rights_added) && same(a.rights_removed, b.rights_removed);
}

export function effectiveRights(roleRights: ReadonlySet<string>, g: Grant): Set<string> {
  const out = new Set(roleRights);
  for (const r of g.rights_added) out.add(r);
  for (const r of g.rights_removed) out.delete(r);
  return out;
}

export function applyChanges(roleRights: ReadonlySet<string>, g: Grant, changes: readonly RightChange[], order: readonly string[]): Grant {
  const added = new Set(g.rights_added);
  const removed = new Set(g.rights_removed);
  for (const { code, granted } of changes) {
    if (roleRights.has(code)) {
      if (granted) removed.delete(code);
      else removed.add(code);
    } else if (granted) added.add(code);
    else added.delete(code);
  }
  const sorted = (s: Set<string>) => order.filter((r) => s.has(r));
  return { role: g.role, rights_added: sorted(added), rights_removed: sorted(removed) };
}

export function grantLabel(g: Pick<GrantView, "role" | "rights_added" | "rights_removed">): string {
  return g.rights_added.length || g.rights_removed.length ? t("kaeso.rights.customized", "{role} · customized", { role: g.role.name }) : g.role.name;
}
