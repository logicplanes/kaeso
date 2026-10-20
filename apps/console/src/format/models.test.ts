import { describe, expect, it } from "vitest";
import type { CatalogueModelView } from "~/api/generated/types";
import { modelName } from "./models";

const model = (display: string | null) => ({ id: "cm_01", model_id: "gpt-5-mini", effective: { display_name: display } }) as unknown as CatalogueModelView;

describe("model names", () => {
  it("show a model's display name, and fall back to its id", () => {
    expect(modelName(model("GPT-5 mini"))).toBe("GPT-5 mini");
    expect(modelName(model(""))).toBe("gpt-5-mini");
    expect(modelName(model(null))).toBe("gpt-5-mini");
  });
});
