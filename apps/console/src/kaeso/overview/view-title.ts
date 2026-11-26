import { t } from "~/i18n";
import type { Overview } from "./data";
import { dimensionTitle } from "./dimension";
import type { Dimension } from "./metrics";
import { efficiencyTitle } from "./sections";

export function viewTitle(view: string, dim: Dimension | undefined, o: Pick<Overview, "showCost">): string {
  if (dim) return dimensionTitle(dim);
  if (view === "reliability") return t("kaeso.overview.reliability", "Reliability");
  if (view === "efficiency") return efficiencyTitle(o);
  return t("kaeso.analytics.breakdown", "Breakdown");
}
