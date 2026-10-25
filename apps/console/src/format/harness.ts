import { agentHarness, harnessName } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export const harnessLabel = (harness: string): string => (agentHarness(harness) === "other" ? t("format.harness.other", "Other") : harnessName(harness));
