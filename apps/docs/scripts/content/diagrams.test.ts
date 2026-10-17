import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { diagramKey, mermaidVersion, type DiagramJob } from "./diagram.ts";
import { browserRender, drawDiagrams, missingDiagrams, syncDiagrams, type Render } from "./diagrams.ts";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

const svg = (key: string, theme: string) => `<svg role="img" width="120" height="40"><title>${key}</title><text>${theme}</text></svg>`;
const drawn: Render = async (sources) => sources.map(({ key }) => ({ key, ok: true, svg: { light: svg(key, "light"), dark: svg(key, "dark") } }));

function folder() {
  root = mkdtempSync(join(tmpdir(), "kaeso-diagrams-"));
  return join(root, "diagrams");
}
const job = (key: string, file = "content/en/index.md", line = 7): DiagramJob => ({ key, code: `flowchart LR\n  accTitle: ${key}\n  a --> b`, file, line });

describe("syncDiagrams", () => {
  it("draws every diagram that has no drawing yet, once per diagram, in both themes", async () => {
    const dir = folder();
    const asked: string[][] = [];
    const render: Render = async (sources) => {
      asked.push(sources.map((s) => s.key));
      return drawn(sources);
    };
    const result = await syncDiagrams(dir, [job("a"), job("b"), job("a", "content/de/index.md")], render);
    expect(asked).toEqual([["a", "b"]]);
    expect(result).toEqual({ drawn: 2, removed: [], problems: [] });
    expect(readdirSync(dir).sort()).toEqual(["a.dark.svg", "a.light.svg", "b.dark.svg", "b.light.svg"]);
    expect(readFileSync(join(dir, "a.dark.svg"), "utf8")).toBe(`${svg("a", "dark")}\n`);
    const again = await syncDiagrams(dir, [job("a"), job("b")], render);
    expect(again.drawn).toBe(0);
    expect(asked).toHaveLength(1);
  });

  it("removes drawings no page uses, unless asked to keep them", async () => {
    const dir = folder();
    await syncDiagrams(dir, [job("a"), job("old")], drawn);
    expect((await syncDiagrams(dir, [job("a")], drawn, { prune: false })).removed).toEqual([]);
    expect(existsSync(join(dir, "old.light.svg"))).toBe(true);
    expect((await syncDiagrams(dir, [job("a")], drawn)).removed).toEqual(["old.dark.svg", "old.light.svg"]);
    expect(readdirSync(dir).sort()).toEqual(["a.dark.svg", "a.light.svg"]);
  });

  it("names the file and line of a diagram that doesn't parse, in every page that has it, and draws nothing for it", async () => {
    const dir = folder();
    const render: Render = async (sources) =>
      sources.map(({ key }) =>
        key === "bad"
          ? { key, ok: false, line: 3, message: "Parse error on line 3: expecting a node" }
          : { key, ok: false, message: "No diagram type detected" },
      );
    const result = await syncDiagrams(dir, [job("bad", "content/en/a.md", 12), job("bad", "content/de/a.md", 14), job("odd", "content/en/b.md", 4)], render);
    expect(result.problems).toEqual([
      "content/en/a.md:15: the diagram doesn't parse: Parse error on line 3: expecting a node",
      "content/de/a.md:17: the diagram doesn't parse: Parse error on line 3: expecting a node",
      "content/en/b.md:5: the diagram doesn't parse: No diagram type detected",
    ]);
    expect(result.drawn).toBe(0);
    expect(existsSync(dir)).toBe(false);
  });

  it("asks the browser for nothing when nothing is missing", async () => {
    expect(await browserRender(process.cwd())([])).toEqual([]);
  });

  it("counts a diagram missing one theme's drawing as missing", () => {
    const dir = folder();
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "a.light.svg"), "");
    writeFileSync(join(dir, "b.light.svg"), "");
    writeFileSync(join(dir, "b.dark.svg"), "");
    expect(missingDiagrams(dir, [job("a"), job("b")]).map((j) => j.key)).toEqual(["a"]);
  });
});

describe("drawDiagrams", () => {
  const docsRoot = () => {
    root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
    const block = "```mermaid\nflowchart LR\n  accTitle: Route\n  app --> kaeso\n```";
    for (const lang of ["en", "de", "zh-CN"]) {
      const file = join(root, "content", lang, "index.md");
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, `---\ntitle: Start\ndescription: About start.\n---\n\nText.\n\n${block}\n`);
    }
    return root;
  };

  it("draws the diagrams of every language and says how many", async () => {
    const r = docsRoot();
    const result = await drawDiagrams(r, "content", drawn, "1.4.0");
    expect(result).toEqual({ ok: true, lines: ["diagrams: 1 in diagrams/, 1 drawn, 0 removed"] });
    const key = diagramKey("flowchart LR\n  accTitle: Route\n  app --> kaeso", mermaidVersion(r));
    expect(readdirSync(join(r, "diagrams")).sort()).toEqual([`${key}.dark.svg`, `${key}.light.svg`]);
  });

  it("fails with each broken diagram's file and line", async () => {
    const r = docsRoot();
    const result = await drawDiagrams(
      r,
      "content",
      async (sources) => sources.map(({ key }) => ({ key, ok: false, line: 2, message: "Lexical error" })),
      "1.4.0",
    );
    expect(result.ok).toBe(false);
    expect(result.lines).toEqual([
      "diagrams: 3 to fix",
      "  content/en/index.md:10: the diagram doesn't parse: Lexical error",
      "  content/de/index.md:10: the diagram doesn't parse: Lexical error",
      "  content/zh-CN/index.md:10: the diagram doesn't parse: Lexical error",
    ]);
  });
});
