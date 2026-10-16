import { describe, expect, it } from "vitest";
import * as ui from "./index.js";

describe("variant exports", () => {
  const entries = Object.entries(ui).filter(([k]) => /^KAESO_.+_VARIANTS$/.test(k) && !k.endsWith("_DEFAULT_VARIANTS"));
  it("exist", () => expect(entries.length).toBeGreaterThan(50));
  for (const [name, variants] of entries) {
    it(name, () => {
      for (const [axis, keys] of Object.entries(variants as Record<string, Record<string, { classes: string; description: string }>>)) {
        for (const [key, v] of Object.entries(keys)) {
          expect(typeof v.classes, `${name}.${axis}.${key}.classes`).toBe("string");
          expect(v.description.length, `${name}.${axis}.${key}.description`).toBeGreaterThan(2);
        }
      }
      const defaults = (ui as Record<string, unknown>)[name.replace(/_VARIANTS$/, "_DEFAULT_VARIANTS")] as Record<string, string> | undefined;
      for (const [axis, key] of Object.entries(defaults ?? {}))
        expect(Object.keys((variants as Record<string, object>)[axis] ?? {}), `${name} default ${axis}`).toContain(String(key));
    });
  }
});
