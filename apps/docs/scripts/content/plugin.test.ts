import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { Plugin, Rollup } from "vite";
import { afterEach, describe, expect, it, vi } from "vitest";
import { diagramKey, mermaidVersion } from "./diagram.ts";
import type { Render } from "./diagrams.ts";
import { buildDocs } from "./docs.ts";
import { absoluteUrl, alternatesFor, htmlFiles, kaesoDocs, pageHtml, pageModuleId, sitemap, structuredData, type PageHead } from "./plugin.ts";

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
  buildStart: (this: { addWatchFile: (f: string) => void; error: (m: string) => never }) => Promise<void>;
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
      image: "https://docs.kaeso.app/og.png",
      indexed: true,
      structured: null,
    };
    const html = pageHtml(TEMPLATE, head);
    expect(html).toContain('<html lang="de">');
    expect(html).toContain("<title>Keys &amp; &quot;limits&quot; &lt;b&gt;</title>");
    expect(html).toContain('<meta name="description" content="Create &lt;keys&gt;." />');
    expect(html).toContain('<meta property="og:site_name" content="Kaeso-Dokumentation" />');
    expect(html).toContain('<link rel="canonical" href="https://docs.kaeso.app/de/keys" />');
    expect(html).toContain('<link rel="alternate" hreflang="en" href="https://docs.kaeso.app/keys" />');
    expect(html).toContain('<link rel="modulepreload" crossorigin href="/assets/page.js" />');
    expect(html).not.toContain("noindex");
    const missing = pageHtml(TEMPLATE, { ...head, url: "", alternates: [], preloads: [], indexed: false });
    expect(missing).not.toContain("canonical");
    expect(missing).not.toContain("og:url");
    expect(missing).not.toContain("application/ld+json");
    expect(missing).toContain('<meta name="robots" content="noindex" />');
  });

  it("describes the page for link previews in its language, with the others as alternates", () => {
    const html = pageHtml(TEMPLATE, {
      lang: "de",
      title: "Schlüssel · Kaeso-Dokumentation",
      description: "Schlüssel anlegen.",
      siteName: "Kaeso-Dokumentation",
      url: "https://docs.kaeso.app/de/keys",
      alternates: [],
      preloads: [],
      image: "https://docs.kaeso.app/og.png",
      indexed: true,
      structured: { "@type": "TechArticle", headline: "a </script> b" },
    });
    expect(html).toContain('<meta property="og:url" content="https://docs.kaeso.app/de/keys" />');
    expect(html).toContain('<meta property="og:locale" content="de_DE" />');
    expect(html).toContain('<meta property="og:locale:alternate" content="en_US" />');
    expect(html).toContain('<meta property="og:locale:alternate" content="zh_CN" />');
    expect(html).not.toContain('<meta property="og:locale:alternate" content="de_DE" />');
    expect(html).toContain('<meta property="og:image" content="https://docs.kaeso.app/og.png" />');
    expect(html).toContain('<meta property="og:image:width" content="1200" />');
    expect(html).toContain('<meta property="og:image:height" content="630" />');
    expect(html).toContain('<meta property="og:image:alt" content="Kaeso" />');
    expect(html).toContain('<meta name="twitter:title" content="Schlüssel · Kaeso-Dokumentation" />');
    expect(html).toContain('<meta name="twitter:description" content="Schlüssel anlegen." />');
    expect(html).toContain('<meta name="twitter:image" content="https://docs.kaeso.app/og.png" />');
    expect(html).toContain('<script type="application/ld+json">{"@type":"TechArticle","headline":"a \\u003c/script> b"}</script>');
  });

  it("calls a language's home page a website and every other page a technical article of it", () => {
    expect(structuredData("https://docs.kaeso.app", "Kaeso-Dokumentation", "de", { path: "/de", title: "Kaeso", description: "Alles über Kaeso." })).toEqual({
      "@context": "https://schema.org",
      "@type": "WebSite",
      name: "Kaeso-Dokumentation",
      url: "https://docs.kaeso.app/de",
      inLanguage: "de",
      description: "Alles über Kaeso.",
    });
    expect(structuredData("https://docs.kaeso.app", "Kaeso documentation", "en", { path: "/guides/keys", title: "Keys", description: "About keys." })).toEqual({
      "@context": "https://schema.org",
      "@type": "TechArticle",
      headline: "Keys",
      description: "About keys.",
      inLanguage: "en",
      url: "https://docs.kaeso.app/guides/keys",
      isPartOf: { "@type": "WebSite", name: "Kaeso documentation", url: "https://docs.kaeso.app/", inLanguage: "en" },
    });
  });

  it("writes a title and description with dollar signs exactly as the author wrote them", () => {
    const html = pageHtml(TEMPLATE, {
      lang: "en",
      title: "Costs in $1 and $& steps",
      description: "Pay $2 or $$ per $' key.",
      siteName: "Kaeso $1 documentation",
      url: "",
      alternates: [],
      preloads: [],
      image: "https://docs.kaeso.app/og.png",
      indexed: true,
      structured: null,
    });
    expect(html).toContain("<title>Costs in $1 and $&amp; steps</title>");
    expect(html).toContain(`<meta name="description" content="Pay $2 or $$ per $' key." />`);
    expect(html).toContain('<meta property="og:title" content="Costs in $1 and $&amp; steps" />');
    expect(html).toContain(`<meta property="og:description" content="Pay $2 or $$ per $' key." />`);
    expect(html).toContain('<meta property="og:site_name" content="Kaeso $1 documentation" />');
  });

  it("lists every page in every language in the sitemap", () => {
    const docs = buildDocs(join(repo(), "content"), root, "1.4.0");
    const xml = sitemap(docs, "https://docs.kaeso.app");
    expect(xml.match(/<loc>/g)).toHaveLength(9);
    expect(xml).toContain("<loc>https://docs.kaeso.app/zh-CN/guides/keys</loc>");
    expect(xml).toContain('<xhtml:link rel="alternate" hreflang="x-default" href="https://docs.kaeso.app/guides" />');
  });
});

describe("diagrams in the Vite plugin", () => {
  const FLOW = "flowchart LR\n  accTitle: Route\n  app[App] --> kaeso[Kaeso]";
  const diagramRepo = (code = FLOW) => {
    root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
    for (const locale of ["en", "de", "zh-CN"]) {
      const file = join(root, "content", locale, "index.md");
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, page("Kaeso", `Text.\n\n\u0060\u0060\u0060mermaid\n${code}\n\u0060\u0060\u0060`));
    }
    return diagramKey(code, mermaidVersion(root));
  };
  const drawn: Render = async (sources) =>
    sources.map(({ key }) => ({ key, ok: true, svg: { light: '<svg width="364" height="49"></svg>', dark: '<svg width="364" height="49"></svg>' } }));
  const plugin = (render: Render) =>
    hooks(
      kaesoDocs({
        root,
        contentDir: "content",
        editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
        siteUrl: "https://docs.kaeso.app",
        version: "1.4.0",
        render,
      }),
    );
  const ctx = {
    addWatchFile: () => undefined,
    error: (m: string): never => {
      throw new Error(m);
    },
  };

  it("draws the missing diagrams before a build, removes unused drawings, and imports both drawings into the page as files", async () => {
    const key = diagramRepo();
    mkdirSync(join(root, "diagrams"));
    writeFileSync(join(root, "diagrams", "ffffffffffffffff.dark.svg"), "<svg></svg>");
    const render = vi.fn(drawn);
    const p = plugin(render);
    p.config({}, { command: "build" });
    await p.buildStart.call(ctx);
    expect(render).toHaveBeenCalledTimes(1);
    expect(existsSync(join(root, "diagrams", `${key}.light.svg`))).toBe(true);
    expect(existsSync(join(root, "diagrams", "ffffffffffffffff.dark.svg"))).toBe(false);
    const module = p.load("\0kaeso-docs:page/de") ?? "";
    expect(module).toContain(`import kaeso_asset_0 from ${JSON.stringify(`${join(root, "diagrams", `${key}.light.svg`)}?url`)};`);
    expect(module).toContain(`import kaeso_asset_1 from ${JSON.stringify(`${join(root, "diagrams", `${key}.dark.svg`)}?url`)};`);
    expect(module).toContain('"t":"diagram","light":kaeso_asset_0,"dark":kaeso_asset_1,"width":364,"height":49,"title":"Route"');
    await p.buildStart.call(ctx);
    expect(render).toHaveBeenCalledTimes(1);
  });

  it("fails a build with the file and line of a diagram that doesn't parse", async () => {
    diagramRepo("flowchart LR\n  accTitle: Route\n  a -->> b");
    const p = plugin(async (sources) => sources.map(({ key }) => ({ key, ok: false, line: 3, message: "Parse error on line 3" })));
    p.config({}, { command: "build" });
    await expect(p.buildStart.call(ctx)).rejects.toThrow(
      "diagrams: 3 to fix\n  content/en/index.md:11: the diagram doesn't parse: Parse error on line 3\n  content/de/index.md:11:",
    );
  });

  it("names the diagrams it couldn't draw when there is no browser", async () => {
    diagramRepo();
    const p = plugin(async () => {
      throw new Error("browserType.launch: Executable doesn't exist\nmore");
    });
    p.config({}, { command: "build" });
    await expect(p.buildStart.call(ctx)).rejects.toThrow(
      "diagrams: 1 to fix\n  can't draw content/en/index.md:8, content/de/index.md:8, content/zh-CN/index.md:8 (browserType.launch: Executable doesn't exist); run `pnpm --filter kaeso-docs diagrams` where Chromium is installed",
    );
  });

  it("draws a missing diagram while serving, then reloads the page", async () => {
    const key = diagramRepo();
    const sent: unknown[] = [];
    const warned: string[] = [];
    let resolve: () => void = () => undefined;
    const done = new Promise<void>((r) => (resolve = r));
    const server = {
      config: { logger: { warn: (m: string) => warned.push(m) } },
      watcher: { add: () => undefined, on: () => undefined },
      moduleGraph: { idToModuleMap: new Map([["\0kaeso-docs:page/en", {}]]), getModuleById: () => ({}), invalidateModule: () => undefined },
      ws: {
        send: (m: unknown) => {
          sent.push(m);
          resolve();
        },
      },
    };
    const p = kaesoDocs({
      root,
      contentDir: "content",
      editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
      siteUrl: "https://docs.kaeso.app",
      version: "1.4.0",
      render: drawn,
    }) as unknown as { configureServer: (s: unknown) => void };
    p.configureServer(server);
    await done;
    expect(sent).toEqual([{ type: "full-reload" }]);
    expect(warned).toEqual([]);
    expect(existsSync(join(root, "diagrams", `${key}.dark.svg`))).toBe(true);
  });

  it("warns about a diagram it couldn't draw while serving, and keeps serving", async () => {
    diagramRepo();
    const warned: string[] = [];
    let resolve: () => void = () => undefined;
    const done = new Promise<void>((r) => (resolve = r));
    const server = {
      config: {
        logger: {
          warn: (m: string) => {
            warned.push(m);
            resolve();
          },
        },
      },
      watcher: { add: () => undefined, on: () => undefined },
      moduleGraph: { idToModuleMap: new Map(), getModuleById: () => undefined, invalidateModule: () => undefined },
      ws: { send: () => undefined },
    };
    const p = kaesoDocs({
      root,
      contentDir: "content",
      editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
      siteUrl: "https://docs.kaeso.app",
      version: "1.4.0",
      render: async (sources) => sources.map(({ key }) => ({ key, ok: false, message: "No diagram type detected" })),
    }) as unknown as { configureServer: (s: unknown) => void };
    p.configureServer(server);
    await done;
    expect(warned[0]).toBe("content/en/index.md:9: the diagram doesn't parse: No diagram type detected");
  });
});

describe("the Vite plugin", () => {
  const plugin = () =>
    kaesoDocs({
      root,
      contentDir: "content",
      editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
      siteUrl: "https://docs.kaeso.app",
      version: "1.4.0",
    });

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

  it("watches every page, and fails a build on a broken page", async () => {
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
    await serve.buildStart.call(ctx);
    expect(watched).toHaveLength(9);
    const build = hooks(plugin());
    build.config({}, { command: "build" });
    await expect(build.buildStart.call(ctx)).rejects.toThrow(/content\/de\/guides\/keys\.md: links to missing\.md, which doesn't exist/);
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
    const notFound = readFileSync(join(outDir, "404.html"), "utf8");
    expect(notFound).not.toContain("canonical");
    expect(notFound).toContain('<meta name="robots" content="noindex" />');
    expect(keys).not.toContain("noindex");
    expect(keys).toContain('"@type":"TechArticle","headline":"Keys"');
    expect(readFileSync(join(outDir, "de.html"), "utf8")).toContain('"@type":"WebSite","name":"Kaeso-Dokumentation","url":"https://docs.kaeso.app/de"');
    expect(readFileSync(join(outDir, "sitemap.xml"), "utf8")).toContain("<loc>https://docs.kaeso.app/de/guides/keys</loc>");
    const { "assets/ui-de.js": _, ...withoutGerman } = bundle;
    expect(() => p.writeBundle({ dir: outDir }, withoutGerman)).toThrow("@logicplanes/kaeso-ui's de messages are not in the bundle");
    const { "assets/site-en.js": __, ...withoutSite } = bundle;
    expect(() => p.writeBundle({ dir: outDir }, withoutSite)).toThrow("the en site module is not in the bundle");
  });

  it("writes a page at a moved address that sends the reader on, and the same moves for hosts that read _redirects", () => {
    repo();
    writeFileSync(join(root, "redirects.json"), JSON.stringify({ "guides/limits": "guides/keys.md#rate-limits", changelog: "https://kaeso.app/changelog" }));
    const outDir = join(root, "dist");
    mkdirSync(outDir, { recursive: true });
    writeFileSync(join(outDir, "index.html"), TEMPLATE);
    const chunk = (fileName: string, facadeModuleId: string | null, isEntry = false) =>
      ({ type: "chunk", fileName, facadeModuleId, imports: [], isEntry }) as unknown as Rollup.OutputChunk;
    const bundle = Object.fromEntries(
      [
        chunk("assets/index.js", "/src/main.tsx", true),
        chunk("assets/site-en.js", "\0kaeso-docs:site/en"),
        chunk("assets/site-de.js", "\0kaeso-docs:site/de"),
        chunk("assets/site-zh.js", "\0kaeso-docs:site/zh-CN"),
        chunk("assets/ui-de.js", "/repo/packages/ui/dist/chunks/de-abc.js"),
        chunk("assets/ui-zh.js", "/repo/packages/ui/dist/chunks/zh-CN-abc.js"),
      ].map((c) => [c.fileName, c]),
    );
    const p = hooks(plugin());
    p.config({}, { command: "build" });
    p.writeBundle({ dir: outDir }, bundle);
    const moved = readFileSync(join(outDir, "de/guides/limits.html"), "utf8");
    expect(moved).toContain('<html lang="de">');
    expect(moved).toContain('<meta name="robots" content="noindex" />');
    expect(moved).toContain('<meta http-equiv="refresh" content="0; url=/de/guides/keys#rate-limits" />');
    expect(moved).toContain('<link rel="canonical" href="https://docs.kaeso.app/de/guides/keys" />');
    expect(moved).toContain('<a href="/de/guides/keys#rate-limits">Keys · Kaeso-Dokumentation</a>');
    const away = readFileSync(join(outDir, "de/changelog.html"), "utf8");
    expect(away).toContain('<meta http-equiv="refresh" content="0; url=https://kaeso.app/de/changelog" />');
    expect(away).toContain('<link rel="canonical" href="https://kaeso.app/de/changelog" />');
    expect(readFileSync(join(outDir, "_redirects"), "utf8")).toBe(
      [
        "/guides/limits /guides/keys#rate-limits 301",
        "/changelog https://kaeso.app/changelog 301",
        "/changelog/* https://kaeso.app/changelog 301",
        "/de/guides/limits /de/guides/keys#rate-limits 301",
        "/de/changelog https://kaeso.app/de/changelog 301",
        "/de/changelog/* https://kaeso.app/de/changelog 301",
        "/zh-CN/guides/limits /zh-CN/guides/keys#rate-limits 301",
        "/zh-CN/changelog https://kaeso.app/zh-CN/changelog 301",
        "/zh-CN/changelog/* https://kaeso.app/zh-CN/changelog 301",
        "",
      ].join("\n"),
    );
    expect(readFileSync(join(outDir, "sitemap.xml"), "utf8")).not.toContain("guides/limits");
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
