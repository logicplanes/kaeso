import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { font, neutral } from "@logicplanes/kaeso-tokens";
import { DIAGRAM_RADIUS, diagramFile, diagramFiles, diagramKey, diagramText, mermaidConfig, mermaidVersion, svgLabels, svgSize } from "./diagram.ts";

let dir = "";
afterEach(() => dir && rmSync(dir, { recursive: true, force: true }));

const FLOW = "flowchart TD\n  accTitle: Route of a request\n  accDescr: An app calls Kaeso.\n  app[App] --> kaeso[Kaeso]";

describe("a diagram's words", () => {
  it("reads the title and a one-line description for screen readers", () => {
    expect(diagramText(FLOW)).toEqual({ title: "Route of a request", description: "An app calls Kaeso." });
  });

  it("joins a description written as a block into one sentence", () => {
    expect(diagramText("sequenceDiagram\n  accTitle: Keys\n  accDescr {\n    The app sends a key.\n    Kaeso checks it.\n  }\n  A->>B: hi")).toEqual({
      title: "Keys",
      description: "The app sends a key. Kaeso checks it.",
    });
  });

  it("has neither when the author wrote neither", () => {
    expect(diagramText("flowchart LR\n  a --> b")).toEqual({});
  });

  it("finds the labels a reader sees, decoded and without markup", () => {
    const svg =
      '<svg width="120" height="40"><text x="0"><tspan>Request &amp; reply</tspan></text><text><tspan>Kaeso</tspan><tspan>gateway</tspan></text><text> </text></svg>';
    expect(svgLabels(svg)).toEqual(["Request & reply", "Kaeso gateway"]);
    expect(svgLabels("<svg><text>&#60;tag&#x3e; &quot;x&quot; &unknown; &nbsp;</text></svg>")).toEqual(['<tag> "x" &unknown;']);
  });

  it("reads a drawing's size from its root element only", () => {
    expect(svgSize('<svg role="img" width="364" height="49.5"><rect width="9" height="9"/></svg>')).toEqual({ width: 364, height: 49.5 });
    expect(svgSize('<svg viewBox="0 0 10 10"><rect width="9" height="9"/></svg>')).toBeNull();
  });
});

describe("drawing names", () => {
  it("names a diagram by its source, the Mermaid version and the site's look, ignoring line endings and outer space", () => {
    const key = diagramKey(FLOW, "11.17.2");
    expect(key).toMatch(/^[0-9a-f]{16}$/);
    expect(diagramKey(`\n${FLOW.replace(/\n/g, "\r\n")}\n\n`, "11.17.2")).toBe(key);
    expect(diagramKey(FLOW.replace("App", "Apps"), "11.17.2")).not.toBe(key);
    expect(diagramKey(FLOW, "11.18.0")).not.toBe(key);
    expect(diagramFile(key, "dark")).toBe(`${key}.dark.svg`);
  });

  it("lists the drawings in a folder, and none for a folder that isn't there", () => {
    dir = mkdtempSync(join(tmpdir(), "kaeso-diagrams-"));
    for (const f of ["b.light.svg", "a.dark.svg", "notes.txt"]) writeFileSync(join(dir, f), "");
    expect(diagramFiles(dir)).toEqual(["a.dark.svg", "b.light.svg"]);
    expect(diagramFiles(join(dir, "missing"))).toEqual([]);
  });

  it("reads the installed Mermaid version, and says when there is none", () => {
    expect(mermaidVersion(process.cwd())).toMatch(/^\d+\.\d+\.\d+$/);
    expect(mermaidVersion(tmpdir())).toBe("unknown");
  });
});

describe("the look", () => {
  it("draws with Kaeso's tokens in each theme, never Mermaid's own", () => {
    const light = mermaidConfig("light");
    const dark = mermaidConfig("dark");
    const vars = (c: Record<string, unknown>) => c.themeVariables as Record<string, string>;
    expect(light).toMatchObject({
      theme: "base",
      layout: "dagre",
      look: "classic",
      securityLevel: "strict",
      htmlLabels: false,
      fontFamily: font.sans,
      fontSize: 14,
      flowchart: { minNodeWidth: 0 },
      state: { minNodeWidth: 0 },
    });
    expect(vars(light)).toMatchObject({
      primaryColor: neutral["bg-raised"].light,
      primaryTextColor: neutral.text.light,
      lineColor: neutral["line-field"].light,
      background: neutral["fill-1"].light,
      edgeLabelBackground: neutral["fill-1"].light,
    });
    expect(vars(dark)).toMatchObject({ primaryColor: neutral["bg-raised"].dark, primaryTextColor: neutral.text.dark, darkMode: true });
    expect(DIAGRAM_RADIUS).toBe(10);
  });

  it("draws an accent node in ink and an outside node dashed, in each theme", () => {
    for (const theme of ["light", "dark"] as const) {
      const css = mermaidConfig(theme).themeCSS as string;
      expect(css).toContain(`.node.accent rect`);
      expect(css).toContain(`fill:${neutral.ink[theme]} !important;stroke:${neutral.ink[theme]} !important`);
      expect(css).toContain(`.node.accent text,.node.accent tspan{fill:${neutral["on-ink"][theme]} !important`);
      expect(css).toMatch(/\.node\.outside rect[^{]*\{[^}]*stroke-dasharray:4 3/);
    }
  });
});
