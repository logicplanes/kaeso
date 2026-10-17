import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { buildSite } from "./site.ts";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

function repo(files: Record<string, string | Buffer>, version = "1.4.0") {
  root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
  for (const [name, text] of Object.entries(files)) {
    const file = join(root, "content", name);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  return buildSite(join(root, "content"), root, { prefix: "", labels: { home: "Introduction", overview: "Overview" }, version });
}
const page = (title: string, body = "", extra = "") => `---\ntitle: ${title}\ndescription: About ${title.toLowerCase()}.\n${extra}---\n\n${body}\n`;
const PNG = Buffer.concat([Buffer.from("89504e470d0a1a0a0000000d49484452", "hex"), Buffer.from([0, 0, 0, 24, 0, 0, 0, 12, 8, 6, 0, 0, 0])]);

describe("buildSite", () => {
  it("makes addresses, the sidebar and previous and next from the folders", () => {
    const site = repo({
      "index.md": page("Kaeso documentation"),
      "install.md": page("Install", "", "order: 1\n"),
      "faq.md": page("Questions"),
      "guides/index.md": page("Guides", "", "order: 1\n"),
      "guides/provider-keys.md": page("Provider keys", "", "order: 2\n"),
      "guides/keys.md": page("Keys", "", "order: 1\n"),
      "guides/limits/index.md": page("Limits", "", "order: 3\nnav: All limits\n"),
      "guides/limits/budgets.md": page("Budgets"),
      "reference/models.md": page("Models"),
    });
    expect(site.problems).toEqual([]);
    expect(site.nav).toEqual([
      {
        id: "pages",
        items: [
          { path: "/", label: "Introduction", depth: 0 },
          { path: "/install", label: "Install", depth: 0 },
          { path: "/faq", label: "Questions", depth: 0 },
        ],
      },
      {
        id: "guides",
        label: "Guides",
        items: [
          { path: "/guides", label: "Overview", depth: 0 },
          { path: "/guides/keys", label: "Keys", depth: 0 },
          { path: "/guides/provider-keys", label: "Provider keys", depth: 0 },
          { path: "/guides/limits", label: "All limits", depth: 0 },
          { path: "/guides/limits/budgets", label: "Budgets", depth: 1 },
        ],
      },
      { id: "reference", label: "Reference", items: [{ path: "/reference/models", label: "Models", depth: 0 }] },
    ]);
    const keys = site.metas.find((m) => m.path === "/guides/keys");
    expect(keys).toMatchObject({ file: "content/guides/keys.md", prev: { path: "/guides", title: "Guides" }, next: { path: "/guides/provider-keys" } });
    expect(site.metas[0]?.prev).toBeUndefined();
    expect(site.metas.at(-1)?.next).toBeUndefined();
    expect(site.search.find((s) => s.path === "/reference/models")?.section).toBe("Reference");
  });

  it("rewrites relative links between Markdown files to the site's addresses, as GitHub follows them", () => {
    const site = repo({
      "index.md": page("Home", "[keys](guides/keys.md) [limits](guides/keys.md#rate-limits) [folder](./guides/) [root](/content/guides/keys.md)"),
      "guides/index.md": page("Guides", "[home](../index.md) [self](#top-level)\n\n## Top level"),
      "guides/keys.md": page("Keys", "## Rate limits\n\n[guides](index.md) [encoded](../guides/keys.md#rate-limits)"),
    });
    expect(site.problems).toEqual([]);
    const hrefs = (path: string) => JSON.stringify(site.pages.find((p) => p.path === path)?.compiled?.blocks).match(/"href":"[^"]*"/g);
    expect(hrefs("/")).toEqual(['"href":"/guides/keys"', '"href":"/guides/keys#rate-limits"', '"href":"/guides"', '"href":"/guides/keys"']);
    expect(hrefs("/guides")).toEqual(['"href":"/"', '"href":"#top-level"']);
    expect(hrefs("/guides/keys")).toEqual(['"href":"/guides"', '"href":"/guides/keys#rate-limits"']);
  });

  it("reports every broken link, image and missing piece with its file", () => {
    const site = repo({
      "guides/keys.md": page(
        "Keys",
        "[a](./missing.md) [b](../../README.md) [c](./limits.md#nowhere) [d](./images/) ![e](./limits.md) ![f](./notes.txt) [ok](./notes.txt)",
      ),
      "guides/limits.md": page("Limits", "## Rate limits"),
      "guides/notes.txt": "notes",
      "guides/deep/page.md": page("Deep"),
    });
    expect(site.problems).toEqual([
      "content/guides/deep: a folder inside a section needs an index.md, which is its page in the sidebar",
      "content/index.md is missing: it is the home page",
      "content/guides/keys.md: links to ./missing.md, which doesn't exist",
      "content/guides/keys.md: links to ../../README.md, which is outside content/",
      "content/guides/keys.md: links to ./limits.md#nowhere, but content/guides/limits.md has no heading with that anchor",
      "content/guides/keys.md: links to ./images/, which doesn't exist",
      "content/guides/keys.md: shows ./limits.md, which is not an image (.png, .jpg, .jpeg, .gif, .webp, .avif, .svg)",
      "content/guides/keys.md: shows ./notes.txt, which is not an image (.png, .jpg, .jpeg, .gif, .webp, .avif, .svg)",
    ]);
    expect([...site.assets]).toEqual(["content/guides/notes.txt"]);
  });

  it("checks titles, addresses and hidden files", () => {
    const site = repo({
      "index.md": page("Home", "# Welcome"),
      "keys!.md": page("Keys"),
      "keys.md": page("Keys again"),
      "_drafts/idea.md": "not a page",
      ".hidden.md": "not a page",
    });
    expect(site.problems.some((p) => p.includes('starts with `# Welcome`, but its title is "Home"'))).toBe(true);
    expect(site.problems.some((p) => /has the same address as .* \(\/keys\)/.test(p))).toBe(true);
    expect(site.pages.map((p) => p.rel).sort()).toEqual(["content/index.md", "content/keys!.md", "content/keys.md"]);
  });

  it("writes the release's version wherever a page says {{version}}", () => {
    const site = repo(
      {
        "index.md": page(
          "Kaeso {{version}}",
          "This documentation describes Kaeso {{version}}, tagged `v{{version}}`.\n\n```bash\ndocker pull ghcr.io/logicplanes/kaeso-gateway:{{version}}\n```",
        ),
      },
      "2.3.0-rc.1",
    );
    expect(site.problems).toEqual([]);
    expect(site.metas[0]?.title).toBe("Kaeso 2.3.0-rc.1");
    const blocks = JSON.stringify(site.pages[0]?.compiled?.blocks);
    expect(blocks).toContain('"v":"This documentation describes Kaeso 2.3.0-rc.1, tagged "},{"t":"code","v":"v2.3.0-rc.1"}');
    expect(blocks).toContain("docker pull ghcr.io/logicplanes/kaeso-gateway:2.3.0-rc.1");
    expect(blocks).not.toContain("{{version}}");
  });

  it("keeps {{version}} itself where a page escapes it with a backslash", () => {
    const site = repo({ "index.md": page("Home", "Write `\\{{version}}` for the version, here {{version}}.") });
    expect(JSON.stringify(site.pages[0]?.compiled?.blocks)).toContain('{"t":"code","v":"{{version}}"},{"t":"text","v":" for the version, here 1.4.0."}');
  });

  it("reads each image's size so the page never shifts as it loads", () => {
    const site = repo({ "index.md": page("Home", "![The flow](./flow.png)"), "flow.png": PNG });
    expect(site.problems).toEqual([]);
    expect(JSON.stringify(site.pages[0]?.compiled?.blocks)).toContain('"src":"content/flow.png","alt":"The flow","width":24,"height":12');
  });
});
