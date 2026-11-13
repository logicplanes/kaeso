import { describe, expect, it } from "vitest";
import { kindLabel } from "./decision-labels";

describe("kindLabel", () => {
  it("names each question type as the console writes it", () => {
    expect(["predicate", "choice", "score", "box"].map((k) => kindLabel(k as "predicate"))).toEqual(["Yes/no", "Choice", "Score", "Box"]);
  });
});
