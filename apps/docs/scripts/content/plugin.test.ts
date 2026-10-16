import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { Plugin, Rollup } from "vite";
import { afterEach, describe, expect, it, vi } from "vitest";
import { buildDocs } from "./docs.ts";
import { absoluteUrl, alternatesFor, htmlFiles, kaesoDocs, pageHtml, pageModuleId, sitemap, type PageHead } from "./plugin.ts";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

const page = (title: string, body = "Text.", extra = "") => `---\ntitle: ${title}\ndescription: About ${title.toLowerCase()}.\n${extra}---\n\n${body}\n`;
const PNG = Buffer.concat([Buffer.from("89504e470d0a1a0a0000000d49484452", "hex"), Buffer.from([0, 0, 0, 24, 0, 0, 0, 12, 8, 6, 0, 0, 0])]);

function repo() {
  root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
  const files: Record<string, string | Buffer> = {};
  for (const locale of ["en", "de", "zh-CN"]) {
    files[`${locale}/index.md`] = page("Kaeso", "See [keys](guides/keys.md).");
    files[`${locale}/guides/index.md`] = page("Guides", "", "order: 1\n");
    files[`${locale}/guides/keys.md`] = page("Keys", "## Rate limits\n\n![The flow](images/flow.png)\n\n[Sample](sample.toml)");
    files[`${locale}/guides/images/flow.png`] = PNG;
    files[`${locale}/guides/sample.toml`] = 'listen = "0.0.0.0:8080"\n';
  }
  for (const [name, text] of Object.entries(files)) {
    const file = join(root, "content", name);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  return root;
}

const TEMPLATE = `<!doctype html>
<html lang="en">
  <head>
    <title>Kaeso documentation</title>
    <meta name="description" content="Guides." />
    <meta property="og:title" content="Kaeso documentation" />
    <meta property="og:description" content="Guides." />
    <meta property="og:site_name" content="Kaeso documentation" />
  </head>
  <body></body>
</html>
`;

type Hooks = {
  config: (c: object, env: { command: "build" | "serve" }) => void;
  buildStart: (this: { addWatchFile: (f: string) => void; error: (m: string) => never }) => void;
  resolveId: (id: string) => string | null;
  load: (id: string) => string | null;
  writeBundle: (output: { dir?: string }, bundle: Rollup.OutputBundle) => void;
};
const hooks = (plugin: Plugin) => plugin as unknown as Hooks;

describe("addresses and HTML", () => {
  it("names a page's module by its language and path", () => {
    expect(pageModuleId("/")).toBe("page/en");
    expect(pageModuleId("/guides/keys")).toBe("page/en/guides/keys");
    expect(pageModuleId("/de")).toBe("page/de");
    expect(pageModuleId("/zh-CN/guides")).toBe("page/zh-CN/guides");
  });

  it("writes a page's absolute address and its alternates in every language", () => {
    expect(absoluteUrl("https://docs.kaeso.app/", "/api")).toBe("https://docs.kaeso.app/api");
    expect(alternatesFor("https://docs.kaeso.app", "/de/api")).toEqual([
      { hreflang: "en", href: "https://docs.kaeso.app/api" },
      { hreflang: "de", href: "https://docs.kaeso.app/de/api" },
      { hreflang: "zh-CN", href: "https://docs.kaeso.app/zh-CN/api" },
      { hreflang: "x-default", href: "https://docs.kaeso.app/api" },
    ]);
    expect(alternatesFor("https://docs.kaeso.app", "/zh-CN")[1]).toEqual({ hreflang: "de", href: "https://docs.kaeso.app/de" });
  });

  it("writes one HTML file per page, and two for a folder's page", () => {
    expect(htmlFiles("/", true)).toEqual(["index.html"]);
    expect(htmlFiles("/guides/keys", false)).toEqual(["guides/keys.html"]);
    expect(htmlFiles("/de/guides", true)).toEqual(["de/guides.html", "de/guides/index.html"]);
  });

  it("fills the template with the page's language, title and description, escaped", () => {
    const head: PageHead = {
      lang: "de",
      title: 'Keys & "limits" <b>',
      description: "Create <keys>.",
      siteName: "Kaeso-Dokumentation",
      url: "https://docs.kaeso.app/de/keys",
      alternates: [{ hreflang: "en", href: "https://docs.kaeso.app/keys" }],
      preloads: ["assets/page.js"],
    };
    const html = pageHtml(TEMPLATE, head);
    expect(html).toContain('<html lang="de">');
    expect(html).toContain("<title>Keys &amp; &quot;limits&quot; &lt;b&gt;</title>");
    expect(html).toContain('<meta name="description" content="Create &lt;keys&gt;." />');
    expect(html).toContain('<meta property="og:site_name" content="Kaeso-Dokumentation" />');
    expect(html).toContain('<link rel="canonical" href="https://docs.kaeso.app/de/keys" />');
    expect(html).toContain('<link rel="alternate" hreflang="en" href="https://docs.kaeso.app/keys" />');
    expect(html).toContain('<link rel="modulepreload" crossorigin href="/assets/page.js" />');
    expect(pageHtml(TEMPLATE, { ...head, url: "", alternates: [], preloads: [] })).not.toContain("canonical");
  });

  it("lists every page in every language in the sitemap", () => {
    const docs = buildDocs(join(repo(), "content"), root);
    const xml = sitemap(docs, "https://docs.kaeso.app");
    expect(xml.match(/<loc>/g)).toHaveLength(9);
    expect(xml).toContain("<loc>https://docs.kaeso.app/zh-CN/guides/keys</loc>");
    expect(xml).toContain('<xhtml:link rel="alternate" hreflang="x-default" href="https://docs.kaeso.app/guides" />');
  });
});

describe("the Vite plugin", () => {
  const plugin = () =>
    kaesoDocs({ root, contentDir: "content", editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/", siteUrl: "https://docs.kaeso.app" });

  it("serves each language's site, search and pages as virtual modules", () => {
    repo();
    const p = hooks(plugin());
    p.config({}, { command: "serve" });
    expect(p.resolveId("virtual:kaeso-docs/site/de")).toBe("\0kaeso-docs:site/de");
    expect(p.resolveId("react")).toBeNull();
    const site = p.load("\0kaeso-docs:site/de") ?? "";
    expect(site).toContain('export const editBase = "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/";');
    expect(site).toContain('"/de/guides/keys": () => import("virtual:kaeso-docs/page/de/guides/keys")');
    expect(JSON.parse((p.load("\0kaeso-docs:search/en") ?? "").replace(/^export default /, "").replace(/;\n$/, ""))[0]).toMatchObject({
      path: "/",
      title: "Kaeso",
    });
    const keys = p.load("\0kaeso-docs:page/en/guides/keys") ?? "";
    expect(keys).toMatch(/^import kaeso_asset_0 from ".*content\/en\/guides\/images\/flow\.png\?url";/);
    expect(keys).toContain('"src":kaeso_asset_0');
    expect(keys).toContain('"href":kaeso_asset_1');
    expect(p.load("\0kaeso-docs:page/en/nothing")).toBe("export default { blocks: [], toc: [] };\n");
    expect(p.load("\0kaeso-docs:site/fr")).toBeNull();
    expect(p.load("\0kaeso-docs:other/en")).toBeNull();
    expect(p.load("/src/main.tsx")).toBeNull();
  });

  it("watches every page, and fails a build on a broken page", () => {
    repo();
    writeFileSync(join(root, "content/de/guides/keys.md"), page("Schlüssel", "[kaputt](missing.md)"));
    const watched: string[] = [];
    const ctx = {
      addWatchFile: (f: string) => watched.push(f),
      error: (m: string): never => {
        throw new Error(m);
      },
    };
    const serve = hooks(plugin());
    serve.config({}, { command: "serve" });
    serve.buildStart.call(ctx);
    expect(watched).toHaveLength(9);
    const build = hooks(plugin());
    build.config({}, { command: "build" });
    expect(() => build.buildStart.call(ctx)).toThrow(/content\/de\/guides\/keys\.md: links to missing\.md, which doesn't exist/);
  });

  it("writes an HTML file per page, a 404 per language and the sitemap", () => {
    repo();
    const outDir = join(root, "dist");
    mkdirSync(outDir, { recursive: true });
    writeFileSync(join(outDir, "index.html"), TEMPLATE);
    const chunk = (fileName: string, facadeModuleId: string | null, imports: string[] = [], isEntry = false) =>
      ({ type: "chunk", fileName, facadeModuleId, imports, isEntry }) as unknown as Rollup.OutputChunk;
    const bundle = Object.fromEntries(
      [
        chunk("assets/index.js", "/src/main.tsx", ["assets/framework.js"], true),
        chunk("assets/framework.js", null),
        chunk("assets/site-en.js", "\0kaeso-docs:site/en"),
        chunk("assets/site-de.js", "\0kaeso-docs:site/de", ["assets/shared.js"]),
        chunk("assets/site-zh.js", "\0kaeso-docs:site/zh-CN"),
        chunk("assets/shared.js", null),
        chunk("assets/ui-de.js", "/repo/packages/ui/dist/chunks/de-abc.js"),
        chunk("assets/ui-zh.js", "/repo/packages/ui/dist/chunks/zh-CN-abc.js"),
        chunk("assets/keys-de.js", "\0kaeso-docs:page/de/guides/keys", ["assets/framework.js"]),
      ].map((c) => [c.fileName, c]),
    );
    const p = hooks(plugin());
    p.config({}, { command: "build" });
    p.writeBundle({ dir: outDir }, bundle);
    const keys = readFileSync(join(outDir, "de/guides/keys.html"), "utf8");
    expect(keys).toContain('<html lang="de">');
    expect(keys).toContain("<title>Keys · Kaeso-Dokumentation</title>");
    expect(keys).toContain('href="/assets/site-de.js"');
    expect(keys).toContain('href="/assets/shared.js"');
    expect(keys).toContain('href="/assets/ui-de.js"');
    expect(keys).toContain('href="/assets/keys-de.js"');
    expect(keys).not.toContain('href="/assets/framework.js"');
    expect(existsSync(join(outDir, "guides.html")) && existsSync(join(outDir, "guides/index.html"))).toBe(true);
    expect(readFileSync(join(outDir, "zh-CN/404.html"), "utf8")).toContain("<title>未找到 · Kaeso 文档</title>");
    expect(readFileSync(join(outDir, "404.html"), "utf8")).not.toContain("canonical");
    expect(readFileSync(join(outDir, "sitemap.xml"), "utf8")).toContain("<loc>https://docs.kaeso.app/de/guides/keys</loc>");
    const { "assets/ui-de.js": _, ...withoutGerman } = bundle;
    expect(() => p.writeBundle({ dir: outDir }, withoutGerman)).toThrow("@logicplanes/kaeso-ui's de messages are not in the bundle");
    const { "assets/site-en.js": __, ...withoutSite } = bundle;
    expect(() => p.writeBundle({ dir: outDir }, withoutSite)).toThrow("the en site module is not in the bundle");
  });

  it("reloads the pages in development when a file in content/ changes", () => {
    repo();
    const listeners: Record<string, (file: string) => void> = {};
    const mod = { id: "\0kaeso-docs:site/en" };
    const server = {
      config: { logger: { warn: vi.fn() } },
      watcher: { add: vi.fn(), on: (event: string, fn: (file: string) => void) => (listeners[event] = fn) },
      moduleGraph: {
        idToModuleMap: new Map<string, object>([
          [mod.id, mod],
          ["/src/main.tsx", {}],
        ]),
        getModuleById: (id: string) => (id === mod.id ? mod : undefined),
        invalidateModule: vi.fn(),
      },
      ws: { send: vi.fn() },
    };
    const p = plugin() as unknown as { configureServer: (s: typeof server) => void };
    p.configureServer(server);
    expect(server.watcher.add).toHaveBeenCalledWith(join(root, "content"));
    expect(server.config.logger.warn).not.toHaveBeenCalled();
    listeners.change?.(join(root, "src/main.tsx"));
    expect(server.ws.send).not.toHaveBeenCalled();
    writeFileSync(join(root, "content/en/guides/keys.md"), page("Keys", "[broken](nowhere.md)"));
    listeners.change?.(join(root, "content/en/guides/keys.md"));
    expect(server.moduleGraph.invalidateModule).toHaveBeenCalledWith(mod);
    expect(server.ws.send).toHaveBeenCalledWith({ type: "full-reload" });
    expect(server.config.logger.warn).toHaveBeenCalledWith(expect.stringContaining("links to nowhere.md"));
  });
});
