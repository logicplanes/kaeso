import type { CatalogueModelView, RouteStep } from "~/api/generated/types";
import { modelName } from "~/format/models";
import { t } from "~/i18n";

function chain(route: RouteStep): string[] | undefined {
  const out: string[] = [];
  let step: RouteStep | null | undefined = route;
  while (step) {
    if (step.type !== "model") return undefined;
    out.push(step.model);
    step = step.fallback;
  }
  return out;
}

function shape(route: RouteStep): { conditions: number; splits: number; models: Set<string> } {
  const count = { conditions: 0, splits: 0, models: new Set<string>() };
  const visit = (s: RouteStep | null | undefined) => {
    if (!s) return;
    if (s.type === "model") {
      count.models.add(s.model);
      return visit(s.fallback);
    }
    if (s.type === "condition") {
      count.conditions++;
      visit(s.then);
      visit(s.else);
      return;
    }
    count.splits++;
    for (const b of s.branches) visit(b.step);
  };
  visit(route);
  return count;
}

export function routeSummary(route: RouteStep, models: ReadonlyMap<string, CatalogueModelView>): string {
  const name = (id: string) => {
    const m = models.get(id);
    return m ? modelName(m) : t("models.route.removedModel", "Removed model");
  };
  const steps = chain(route);
  if (steps) return steps.map(name).join(" → ");
  const found = shape(route);
  const count = found.models.size;
  if (found.conditions > 0 && found.splits > 0)
    return t("models.alias.byBoth", "Routes to {count, plural, one {# model} other {# models}} by conditions and splits", { count });
  if (found.conditions > 0) return t("models.alias.byConditions", "Routes to {count, plural, one {# model} other {# models}} by conditions", { count });
  return t("models.alias.bySplits", "Splits requests across {count, plural, one {# model} other {# models}}", { count });
}
