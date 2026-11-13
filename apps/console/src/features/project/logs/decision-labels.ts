import type { DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { t } from "~/i18n";

export function kindLabel(kind: DecisionQuestion["kind"]): string {
  switch (kind) {
    case "predicate":
      return t("logs.decision.kind.predicate", "Yes/no");
    case "choice":
      return t("logs.decision.kind.choice", "Choice");
    case "score":
      return t("logs.decision.kind.score", "Score");
    case "box":
      return t("logs.decision.kind.box", "Box");
  }
}
