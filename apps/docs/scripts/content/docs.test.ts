import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { checkContent } from "./check.ts";
import { buildDocs, kaesoVersion } from "./docs.ts";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

function repo(files: Record<string, string>) {
  root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
  for (const [name, text] of Object.entries(files)) {
    const file = join(root, "content", name);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  return root;
}
const page = (title: string, extra = "") => `---\ntitle: ${title}\ndescription: About ${title.toLowerCase()}.\n${extra}---\n\nText.\n`;
const everyLanguage = (files: Record<string, string>) =>
  Object.fromEntries(["en", "de", "zh-CN"].flatMap((l) => Object.entries(files).map(([name, text]) => [`${l}/${name}`, text])));

describe("buildDocs", () => {
  it("builds one site per language with the language's prefix and labels", () => {
    const docs = buildDocs(join(repo(everyLanguage({ "index.md": page("Start"), "guides/index.md": page("Guides", "order: 1\n") })), "content"), root, "1.4.0");
    expect(docs.problems).toEqual([]);
    expect(docs.sites.en.metas.map((m) => m.path)).toEqual(["/", "/guides"]);
    expect(docs.sites.de.metas.map((m) => m.path)).toEqual(["/de", "/de/guides"]);
    expect(docs.sites["zh-CN"].nav[0]?.items[0]).toEqual({ path: "/zh-CN", label: "简介", depth: 0 });
    expect(docs.sites.de.nav[1]?.items[0]?.label).toBe("Übersicht");
  });

  it("requires the same pages, in the same order, in every language", () => {
    const docs = buildDocs(
      join(
        repo({
          ...everyLanguage({ "index.md": page("Start") }),
          "en/keys.md": page("Keys", "order: 2\n"),
          "de/keys.md": page("Schlüssel", "order: 3\n"),
          "zh-CN/extra.md": page("Extra"),
        }),
        "content",
      ),
      root,
      "1.4.0",
    );
    expect(docs.problems).toEqual([
      "content/de/keys.md: its order is 3, but content/en/keys.md has 2; a translation keeps the English order",
      "content/zh-CN/keys.md: missing; translate content/en/keys.md",
      "content/zh-CN/extra.md: has no English page; add content/en/extra.md or remove it",
    ]);
  });

  it("keeps every page inside a language folder, and language folders out of English addresses", () => {
    const docs = buildDocs(
      join(repo({ ...everyLanguage({ "index.md": page("Start") }), "notes.md": page("Notes"), "en/de/index.md": page("Deutsch") }), "content"),
      root,
      "1.4.0",
    );
    expect(docs.problems).toContain("content/notes.md: put every page in a language folder (en, de, zh-CN)");
    expect(docs.problems.some((p) => p.startsWith("content/en/de/index.md: its address /de is where the Deutsch pages live"))).toBe(true);
  });

  it("names every drawing that no page draws any more", () => {
    repo(everyLanguage({ "index.md": page("Start") }));
    mkdirSync(join(root, "diagrams"));
    writeFileSync(join(root, "diagrams", "0123456789abcdef.light.svg"), "<svg></svg>");
    const docs = buildDocs(join(root, "content"), root, "1.4.0");
    expect(docs.diagramDir).toBe(join(root, "diagrams"));
    expect(docs.diagrams).toEqual([]);
    expect(docs.problems).toEqual([
      "diagrams/0123456789abcdef.light.svg: no page draws this diagram any more; run `pnpm --filter kaeso-docs diagrams` to remove it",
    ]);
  });

  it("names a missing content folder", () => {
    root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
    expect(buildDocs(join(root, "content"), root, "1.4.0").problems).toEqual(["content does not exist"]);
  });
});

describe("kaesoVersion", () => {
  it("is the version of the repository's package.json, which every release sets", () => {
    const docsRoot = join(import.meta.dirname, "..", "..");
    const { version } = JSON.parse(readFileSync(join(docsRoot, "..", "..", "package.json"), "utf8")) as { version: string };
    expect(kaesoVersion(docsRoot)).toBe(version);
  });
});

describe("checkContent", () => {
  it("counts the pages and languages when everything is in order", () => {
    repo(everyLanguage({ "index.md": page("Start"), "keys.md": page("Keys") }));
    expect(checkContent(root, "content", "1.4.0")).toEqual({ ok: true, lines: ["content: 2 pages in 3 languages, every link checked"] });
  });

  it("lists every problem with its file", () => {
    repo({ "en/index.md": page("Start") });
    const result = checkContent(root, "content", "1.4.0");
    expect(result.ok).toBe(false);
    expect(result.lines).toEqual([
      "content: 4 to fix in content/",
      "  content/de does not exist",
      "  content/zh-CN does not exist",
      "  content/de/index.md: missing; translate content/en/index.md",
      "  content/zh-CN/index.md: missing; translate content/en/index.md",
    ]);
  });
});
