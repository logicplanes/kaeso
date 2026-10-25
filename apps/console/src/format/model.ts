import { agentModel, modelName } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export const modelLabel = (model: string): string => (agentModel(model) === "unknown" ? t("format.model.unknown", "Unknown") : modelName(model));
