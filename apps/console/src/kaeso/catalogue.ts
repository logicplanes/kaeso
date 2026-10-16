import { useQuery } from "@tanstack/react-query";
import type { RightSource, RightsGroup } from "@logicplanes/kaeso-ui";
import type { RightView, SourceView, RightsCatalogue, HeldRight } from "~/api/generated/types";
import { rightsCatalogueQuery } from "~/api/queries/rights";
import { t } from "~/i18n";

export type Scope = "org" | "project";

export interface Catalogue {
  scopes: Record<Scope, { id: string; name: string; rights: RightView[] }[]>;
  byId: ReadonlyMap<string, RightView>;
  order: Record<Scope, string[]>;
}

export function indexCatalogue(c: RightsCatalogue): Catalogue {
  const scopes = { org: c.org.categories, project: c.project.categories };
  const byId = new Map<string, RightView>();
  for (const cat of [...scopes.org, ...scopes.project]) for (const r of cat.rights) byId.set(r.id, r);
  const order = { org: scopes.org.flatMap((x) => x.rights.map((r) => r.id)), project: scopes.project.flatMap((x) => x.rights.map((r) => r.id)) };
  return { scopes, byId, order };
}

export function useCatalogue(org: string): Catalogue | undefined {
  return useQuery({ ...rightsCatalogueQuery(org), select: indexCatalogue, enabled: !!org }).data;
}

export const rightName = (cat: Catalogue | undefined, id: string): string => cat?.byId.get(id)?.name ?? id;

export function rightNames(cat: Catalogue | undefined, ids: readonly string[]): string {
  const [first = "", second = ""] = ids.map((id) => rightName(cat, id));
  if (ids.length <= 1) return first;
  if (ids.length === 2) return t("kaeso.rights.namesTwo", "{first} and {second}", { first, second });
  return t("kaeso.rights.namesMore", "{first}, {second} and {count} more", { first, second, count: ids.length - 2 });
}

export function withNeeds(cat: Catalogue, ids: Iterable<string>): Set<string> {
  const out = new Set<string>();
  const queue = [...ids];
  while (queue.length) {
    const id = queue.pop() as string;
    if (out.has(id)) continue;
    out.add(id);
    queue.push(...(cat.byId.get(id)?.needs ?? []));
  }
  return out;
}

export interface GroupOptions {
  granted: ReadonlySet<string>;
  base?: ReadonlySet<string> | undefined;
  held?: ReadonlySet<string> | undefined;
  readOnly?: string | undefined;
  sources?: ReadonlyMap<string, RightSource[]> | undefined;
}

export function rightsGroups(cat: Catalogue, scope: Scope, opts: GroupOptions): RightsGroup[] {
  return cat.scopes[scope].map((category) => ({
    label: category.name,
    rights: category.rights.map((r) => {
      const on = opts.granted.has(r.id);
      const inBase = opts.base?.has(r.id);
      const reason =
        opts.readOnly ?? (opts.held && !opts.held.has(r.id) && !on ? t("kaeso.rights.notHeld", "You don't hold this right, so you can't grant it") : undefined);
      return {
        code: r.id,
        label: r.name,
        description: r.description,
        granted: on,
        sensitive: r.sensitive,
        requires: r.needs,
        ...(opts.base ? { origin: inBase ? (on ? ("preset" as const) : ("removed" as const)) : on ? ("added" as const) : ("none" as const) } : {}),
        ...(opts.sources ? { sources: opts.sources.get(r.id) ?? [] } : {}),
        ...(reason ? { disabled: true, disabledReason: reason } : {}),
      };
    }),
  }));
}

export function sourceChip(cat: Catalogue | undefined, s: SourceView, where: "org" | "project"): RightSource {
  switch (s.kind) {
    case "owner":
      return { label: t("common.owner", "Owner"), detail: t("kaeso.rights.ownerSource", "Owners hold every right") };
    case "added":
      return { label: t("kaeso.rights.added", "Added"), detail: t("kaeso.rights.addedSource", "Given to them on top of their role") };
    case "org_role":
      return {
        label: t("kaeso.rights.orgRoleSource", "{role} · organization", { role: s.role?.name ?? t("kaeso.rights.orgRole", "Org role") }),
        detail: t("kaeso.rights.orgRoleSourceHint", "Their organization role gives this in every project"),
      };
    case "org_right":
      return { label: rightName(cat, s.right ?? ""), detail: t("kaeso.rights.orgRightSource", "An organization right that reaches every project") };
    case "project_role":
      return {
        label: s.role?.name ?? t("kaeso.rights.projectRole", "Project role"),
        detail: t("kaeso.rights.projectRoleSource", "Their role in this project"),
      };
    default:
      return {
        label: s.role?.name ?? t("common.role", "Role"),
        detail: where === "org" ? t("kaeso.rights.orgRoleOnly", "Their organization role") : t("kaeso.rights.projectRoleSource", "Their role in this project"),
      };
  }
}

export function sourcesOf(cat: Catalogue | undefined, rights: readonly HeldRight[], where: "org" | "project"): Map<string, RightSource[]> {
  return new Map(rights.map((r) => [r.right, r.sources.map((s) => sourceChip(cat, s, where))]));
}
