import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { allowed, packageRoot, shippedLicenses } from "./licenses.ts";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

function write(file: string, text: string) {
  mkdirSync(dirname(join(root, file)), { recursive: true });
  writeFileSync(join(root, file), text);
}

describe("the licence check", () => {
  it("finds a package's folder in a path, scoped or not, through pnpm's store", () => {
    expect(packageRoot("/r/node_modules/.pnpm/react@19/node_modules/react/index.js")).toBe("/r/node_modules/.pnpm/react@19/node_modules/react");
    expect(packageRoot("/r/node_modules/@logicplanes/kaeso-ui/dist/index.js")).toBe("/r/node_modules/@logicplanes/kaeso-ui");
    expect(packageRoot("/r/src/main.tsx")).toBeNull();
  });

  it("allows only permissive licences, in SPDX expressions too", () => {
    expect(allowed("MIT")).toBe(true);
    expect(allowed("(MIT OR GPL-3.0)")).toBe(true);
    expect(allowed("Apache-2.0 AND BSD-3-Clause")).toBe(true);
    expect(allowed("(MIT OR Apache-2.0) AND BSD-3-Clause")).toBe(true);
    expect(allowed("(MIT OR GPL-3.0) AND GPL-3.0")).toBe(false);
    expect(allowed("(MIT")).toBe(false);
    expect(allowed("MIT AND GPL-3.0")).toBe(false);
    expect(allowed("Unlicense")).toBe(false);
    expect(allowed("UNKNOWN")).toBe(false);
  });

  it("reads the licence of every package whose code is in the build, and refuses the others", () => {
    root = mkdtempSync(join(tmpdir(), "kaeso-docs-licenses-"));
    write("node_modules/.pnpm/a@1/node_modules/a/package.json", JSON.stringify({ name: "a", version: "1.0.0", license: "MIT" }));
    write("node_modules/@scope/b/package.json", JSON.stringify({ name: "@scope/b", version: "2.0.0", license: { type: "ISC" } }));
    write("node_modules/c/package.json", JSON.stringify({ name: "c", version: "3.0.0", licenses: [{ type: "GPL-3.0" }] }));
    write("node_modules/d/package.json", JSON.stringify({ name: "d", version: "4.0.0" }));
    write(
      "dist/assets/index.js.map",
      JSON.stringify({
        sources: [
          "../../node_modules/.pnpm/a@1/node_modules/a/index.js",
          "../../node_modules/@scope/b/lib/x.js",
          "../../src/main.tsx",
          "../../node_modules/gone/x.js",
        ],
      }),
    );
    write(
      "dist/assets/nested/page.js.map",
      JSON.stringify({ sources: ["../../../node_modules/c/x.js", "../../../node_modules/d/x.js", "../../../node_modules/@scope/b/lib/y.js"] }),
    );
    write("dist/assets/plain.js.map", JSON.stringify({}));
    const report = shippedLicenses(join(root, "dist"));
    expect(report.packages.map((p) => `${p.name} ${p.license}`)).toEqual(["@scope/b ISC", "a MIT", "c GPL-3.0", "d UNKNOWN"]);
    expect(report.refused.map((p) => p.name)).toEqual(["c", "d"]);
    expect(report.lines).toEqual([
      "licenses: 4 packages in the build: ISC 1, MIT 1, GPL-3.0 1, UNKNOWN 1",
      "licenses: c@3.0.0 is GPL-3.0, which is not allowed (MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD)",
      "licenses: d@4.0.0 is UNKNOWN, which is not allowed (MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD)",
    ]);
  });
});
