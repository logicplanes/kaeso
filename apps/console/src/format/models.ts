import type { CatalogueModelView } from "~/api/generated/types";

export const modelName = (m: Pick<CatalogueModelView, "model_id" | "effective">): string => m.effective.display_name || m.model_id;

export function modelOption(m: CatalogueModelView): { value: string; label: string; description?: string } {
  const name = modelName(m);
  return name === m.model_id ? { value: m.id, label: name } : { value: m.id, label: name, description: m.model_id };
}
