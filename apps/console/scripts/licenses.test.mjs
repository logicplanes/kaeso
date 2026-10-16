import { describe, expect, it } from "vitest";
import { allowed } from "./licenses.mjs";

describe("the console's licence check", () => {
  it("allows permissive licences and SPDX expressions made only of them", () => {
    expect(allowed("MIT")).toBe(true);
    expect(allowed("(MIT OR GPL-3.0)")).toBe(true);
    expect(allowed("Apache-2.0 AND BSD-3-Clause")).toBe(true);
    expect(allowed("(MIT OR Apache-2.0) AND BSD-3-Clause")).toBe(true);
  });

  it("refuses an expression that needs a licence it can't ship", () => {
    expect(allowed("(MIT OR GPL-3.0) AND GPL-3.0")).toBe(false);
    expect(allowed("MIT AND GPL-3.0")).toBe(false);
    expect(allowed("UNKNOWN")).toBe(false);
    expect(allowed("(MIT")).toBe(false);
    expect(allowed("")).toBe(false);
  });
});
