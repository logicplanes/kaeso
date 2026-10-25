import type { SegmentedOption } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type Expiry = "7d" | "30d" | "90d" | "never";

export function expiryOptions(): SegmentedOption[] {
  return [
    { value: "7d", label: t("kaeso.agents.expiry7", "7 days") },
    { value: "30d", label: t("kaeso.agents.expiry30", "30 days") },
    { value: "90d", label: t("kaeso.agents.expiry90", "90 days") },
    { value: "never", label: t("kaeso.agents.expiryNever", "Never") },
  ];
}
