import { describe, expect, it } from "vitest";
import type { CatalogueModelView } from "~/api/generated/types";
import { modelName, modelOption } from "./models";

const model = (display: string | null) => ({ id: "cm_01", model_id: "gpt-5-mini", effective: { display_name: display } }) as unknown as CatalogueModelView;

describe("model names", () => {
  it("show a model's display name, with its id beside it in a list", () => {
    expect(modelName(model("GPT-5 mini"))).toBe("GPT-5 mini");
    expect(modelOption(model("GPT-5 mini"))).toEqual({ value: "cm_01", label: "GPT-5 mini", description: "gpt-5-mini" });
  });

  it("fall back to the id, and don't repeat it", () => {
    expect(modelName(model(""))).toBe("gpt-5-mini");
    expect(modelName(model(null))).toBe("gpt-5-mini");
    expect(modelOption(model(null))).toEqual({ value: "cm_01", label: "gpt-5-mini" });
    expect(modelOption(model("gpt-5-mini"))).toEqual({ value: "cm_01", label: "gpt-5-mini" });
  });
});
