import type { ModelPickerOption } from "@logicplanes/kaeso-ui/playground";
import type { CatalogueModelView } from "~/api/generated/types";
import type { PlaygroundModel } from "~/api/queries/playground";
import { capabilities } from "~/features/project/models/model-fields";
import { modelName } from "~/format/models";
import { costUSD } from "~/format/money";
import { recall, remember } from "~/shell/session-memory";

export function pickerPrice(input: string | null | undefined, output: string | null | undefined): string | undefined {
  return input && output ? `${costUSD(input)} / ${costUSD(output)}` : undefined;
}

export function catalogueModelOption(m: CatalogueModelView): ModelPickerOption {
  const name = modelName(m);
  const price = pickerPrice(m.effective.input_price, m.effective.output_price);
  return {
    value: m.id,
    provider: m.provider,
    model: m.model_id,
    ...(name !== m.model_id ? { name } : {}),
    capabilities: capabilities(m.effective),
    ...(m.effective.context_limit ? { context: m.effective.context_limit } : {}),
    ...(price ? { price } : {}),
    ...(m.deprecated ? { deprecated: true } : {}),
  };
}

export function playgroundModelOption(m: PlaygroundModel, value: string, noKey: string): ModelPickerOption {
  const price = pickerPrice(m.input_price, m.output_price);
  return {
    value,
    provider: m.provider,
    model: m.model_id,
    ...(m.display_name !== m.model_id ? { name: m.display_name } : {}),
    capabilities: [
      ...(m.reasoning ? (["reasoning"] as const) : []),
      ...(m.input_modalities.includes("image") ? (["vision"] as const) : []),
      ...(m.input_modalities.includes("pdf") ? (["pdf"] as const) : []),
    ],
    ...(m.context_limit ? { context: m.context_limit } : {}),
    ...(price ? { price } : {}),
    ...(m.provider_key ? {} : { disabled: true, note: noKey }),
  };
}

export function recentModels(project: string): string[] {
  return recall<string[]>(["playground", "recent", project]) ?? [];
}

export function rememberModel(project: string, value: string): void {
  remember(["playground", "recent", project], [value, ...recentModels(project).filter((v) => v !== value)].slice(0, 5));
}
