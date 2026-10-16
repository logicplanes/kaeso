import { Collapsible, EffectiveAccess, RightsMatrix, Skeleton, type RightSource } from "@logicplanes/kaeso-ui";
import type { HeldRight } from "~/api/generated/types";
import { t } from "~/i18n";
import { rightsGroups, sourceChip, sourcesOf, type Catalogue, type Scope } from "./catalogue";

export function AccessView({
  cat,
  scope,
  rights,
  label,
}: {
  cat: Catalogue | undefined;
  scope: Scope;
  rights: readonly HeldRight[] | undefined;
  label: string;
}) {
  if (!cat || !rights) return <Skeleton className="w-full" />;
  const where = scope === "org" ? "org" : "project";
  const granted = new Set(rights.map((r) => r.right));
  const groups = rightsGroups(cat, scope, { granted, sources: sourcesOf(cat, rights, where) });
  const seen = new Map<string, RightSource>();
  for (const r of rights) for (const s of r.sources) if (s.kind !== "added") seen.set(sourceChip(cat, s, where).label, sourceChip(cat, s, where));
  if (rights.some((r) => r.sources.some((s) => s.kind === "added")))
    seen.set("added", { label: t("kaeso.rights.added", "Added"), detail: t("kaeso.rights.addedRights", "Rights given to them on top of their role") });
  return (
    <div className="flex flex-col gap-8">
      <EffectiveAccess aria-label={label} groups={groups} sources={[...seen.values()]} />
      <Collapsible title={t("kaeso.rights.everyRight", "Every right")} hint={t("kaeso.rights.everyRightHint", "With where each comes from")}>
        <RightsMatrix readOnly searchable aria-label={label} groups={groups} />
      </Collapsible>
    </div>
  );
}
