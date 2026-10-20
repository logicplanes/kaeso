import type { CatalogueModelView } from "~/api/generated/types";

export const modelName = (m: Pick<CatalogueModelView, "model_id" | "effective">): string => m.effective.display_name || m.model_id;
