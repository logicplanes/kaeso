import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Plugin, Rollup } from "vite";
import { DEFAULT_LOCALE, LOCALES, homePath, isLocale, localeOfPath, pathWithinLocale, type Locale } from "../../src/i18n/locales.ts";
import { MESSAGES } from "../../src/i18n/messages/index.ts";
import type { Block, Inline, PageContent, PageMeta } from "../../src/markdown/ast.ts";
import { documentTitle } from "../../src/page/document-title.ts";
import { DIAGRAM_COMMAND } from "./diagram.ts";
import { browserRender, missingDiagrams, syncDiagrams, type Render } from "./diagrams.ts";
import { buildDocs, type Docs } from "./docs.ts";
import type { Site, SitePage } from "./site.ts";

export interface DocsOptions {
  root: string;
  contentDir: string;
  editBase: string;
  siteUrl: string;
  version: string;
  render?: Render;
}

export interface PageHead {
  lang: Locale;
  title: string;
  description: string;
  siteName: string;
  url: string;
  alternates: { hreflang: string; href: string }[];
  preloads: string[];
  image: string;
  indexed: boolean;
  structured: object | null;
}

export const OG_IMAGE = { path: "/og.png", width: 1200, height: 630, alt: "Kaeso" } as const;

export const OG_LOCALES: Readonly<Record<Locale, string>> = { en: "en_US", de: "de_DE", "zh-CN": "zh_CN" };

const VIRTUAL = "virtual:kaeso-docs/";
const RESOLVED = "\0kaeso-docs:";

export function pageModuleId(path: string): string {
  const locale = localeOfPath(path);
  const within = pathWithinLocale(path);
  return `page/${locale}${within === "/" ? "" : within}`;
}

export function absoluteUrl(siteUrl: string, path: string): string {
  return `${siteUrl.replace(/\/+$/, "")}${path}`;
}

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const meta = (key: "name" | "property", name: string, content: string | number) => `<meta ${key}="${name}" content="${escapeHtml(String(content))}" />`;

const scriptJson = (value: object) => JSON.stringify(value).replace(/</g, "\\u003c");

export function pageHtml(template: string, head: PageHead): string {
  const title = escapeHtml(head.title);
  const description = escapeHtml(head.description);
  const links = [
    ...(head.indexed ? [] : [meta("name", "robots", "noindex")]),
    ...(head.url ? [`<link rel="canonical" href="${escapeHtml(head.url)}" />`, meta("property", "og:url", head.url)] : []),
    ...head.alternates.map((a) => `<link rel="alternate" hreflang="${a.hreflang}" href="${escapeHtml(a.href)}" />`),
    meta("property", "og:locale", OG_LOCALES[head.lang]),
    ...LOCALES.filter((l) => l !== head.lang).map((l) => meta("property", "og:locale:alternate", OG_LOCALES[l])),
    meta("property", "og:image", head.image),
    meta("property", "og:image:type", "image/png"),
    meta("property", "og:image:width", OG_IMAGE.width),
    meta("property", "og:image:height", OG_IMAGE.height),
    meta("property", "og:image:alt", OG_IMAGE.alt),
    meta("name", "twitter:title", head.title),
    meta("name", "twitter:description", head.description),
    meta("name", "twitter:image", head.image),
    ...(head.structured ? [`<script type="application/ld+json">${scriptJson(head.structured)}</script>`] : []),
    ...head.preloads.map((file) => `<link rel="modulepreload" crossorigin href="/${file}" />`),
  ];
  return template
    .replace(/<html lang="[^"]*">/, () => `<html lang="${head.lang}">`)
    .replace(/<title>[^<]*<\/title>/, () => `<title>${title}</title>`)
    .replace(/(<meta name="description" content=")[^"]*(")/, (_, open: string, close: string) => `${open}${description}${close}`)
    .replace(/(<meta property="og:title" content=")[^"]*(")/, (_, open: string, close: string) => `${open}${title}${close}`)
    .replace(/(<meta property="og:description" content=")[^"]*(")/, (_, open: string, close: string) => `${open}${description}${close}`)
    .replace(/(<meta property="og:site_name" content=")[^"]*(")/, (_, open: string, close: string) => `${open}${escapeHtml(head.siteName)}${close}`)
    .replace("</head>", () => `    ${links.join("\n    ")}\n  </head>`);
}

export function structuredData(siteUrl: string, siteName: string, locale: Locale, page: Pick<PageMeta, "path" | "title" | "description">): object {
  const website = { "@type": "WebSite", name: siteName, url: absoluteUrl(siteUrl, homePath(locale)), inLanguage: locale };
  if (pathWithinLocale(page.path) === "/") return { "@context": "https://schema.org", ...website, description: page.description };
  return {
    "@context": "https://schema.org",
    "@type": "TechArticle",
    headline: page.title,
    description: page.description,
    inLanguage: locale,
    url: absoluteUrl(siteUrl, page.path),
    isPartOf: website,
  };
}

export function alternatesFor(siteUrl: string, path: string): PageHead["alternates"] {
  const within = pathWithinLocale(path);
  const href = (locale: Locale) => absoluteUrl(siteUrl, locale === DEFAULT_LOCALE ? within : `/${locale}${within === "/" ? "" : within}`);
  return [...LOCALES.map((locale) => ({ hreflang: locale, href: href(locale) })), { hreflang: "x-default", href: href(DEFAULT_LOCALE) }];
}

export function sitemap(docs: Docs, siteUrl: string): string {
  const urls = docs.sites[DEFAULT_LOCALE].metas.flatMap((meta) =>
    LOCALES.map((locale) => {
      const alternates = alternatesFor(siteUrl, meta.path);
      const own = alternates.find((a) => a.hreflang === locale)?.href ?? "";
      const links = alternates.map((a) => `    <xhtml:link rel="alternate" hreflang="${a.hreflang}" href="${escapeHtml(a.href)}" />`);
      return [`  <url>`, `    <loc>${escapeHtml(own)}</loc>`, ...links, `  </url>`].join("\n");
    }),
  );
  return [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">`,
    ...urls,
    `</urlset>`,
    "",
  ].join("\n");
}

function withAssets(page: SitePage, root: string): string {
  const refs = new Map<string, string>();
  const ref = (r: string) => {
    const known = refs.get(r);
    if (known) return known;
    const token = `__kaeso_asset_${refs.size}__`;
    refs.set(r, token);
    return token;
  };
  const inline = (xs: Inline[]): Inline[] =>
    xs.map((x) => {
      if (x.t === "img") return { ...x, src: ref(x.src) };
      if (x.t === "link") return { ...x, c: inline(x.c), ...(x.kind === "file" ? { href: ref(x.href) } : {}) };
      if ("c" in x) return { ...x, c: inline(x.c) };
      return x;
    });
  const blocks = (bs: Block[]): Block[] =>
    bs.map((b) => {
      switch (b.t) {
        case "heading":
        case "paragraph":
          return { ...b, c: inline(b.c) };
        case "list":
          return { ...b, items: b.items.map((i) => ({ ...i, c: blocks(i.c) })) };
        case "quote":
        case "callout":
          return { ...b, c: blocks(b.c) };
        case "table":
          return { ...b, head: b.head.map(inline), rows: b.rows.map((r) => r.map(inline)) };
        case "links":
          return { ...b, items: b.items.map((card) => ({ ...card, title: inline(card.title), description: inline(card.description) })) };
        case "diagram":
          return { ...b, light: ref(b.light), dark: ref(b.dark) };
        default:
          return b;
      }
    });
  const compiled = page.compiled ?? { blocks: [], toc: [] };
  const content: PageContent = {
    blocks: blocks(compiled.blocks),
    toc: compiled.toc,
  };
  let json = JSON.stringify(content);
  const imports: string[] = [];
  for (const [r, token] of refs) {
    const id = token.replace(/^__|__$/g, "");
    imports.push(`import ${id} from ${JSON.stringify(`${resolve(root, r)}?url`)};`);
    json = json.split(JSON.stringify(token)).join(id);
  }
  return [...imports, `export default ${json};`, ""].join("\n");
}

function pageModule(docs: Docs, name: string, root: string): string {
  const [, locale = "", ...rest] = name.split("/");
  const site = isLocale(locale) ? docs.sites[locale] : undefined;
  const path = `${locale === DEFAULT_LOCALE ? "" : `/${locale}`}/${rest.join("/")}`.replace(/(.)\/$/, "$1");
  const page = site?.pages.find((p) => p.path === path);
  return page ? withAssets(page, root) : "export default { blocks: [], toc: [] };\n";
}

function siteModule(site: Site, editBase: string): string {
  const loaders = site.metas.map((m) => `  ${JSON.stringify(m.path)}: () => import(${JSON.stringify(`${VIRTUAL}${pageModuleId(m.path)}`)}),`);
  return [
    `export const editBase = ${JSON.stringify(editBase)};`,
    `export const nav = ${JSON.stringify(site.nav)};`,
    `export const pages = ${JSON.stringify(Object.fromEntries(site.metas.map((m) => [m.path, m])))};`,
    `export const load = {`,
    ...loaders,
    `};`,
    "",
  ].join("\n");
}

type Chunk = Rollup.OutputChunk;

function preloads(bundle: Rollup.OutputBundle): { locale: (locale: Locale) => string[]; page: (path: string) => string[] } {
  const chunks = Object.values(bundle).filter((c): c is Chunk => c.type === "chunk");
  const byFile = new Map(chunks.map((c) => [c.fileName, c]));
  const closure = (chunk: Chunk | undefined, into = new Set<string>()): Set<string> => {
    if (!chunk || into.has(chunk.fileName)) return into;
    into.add(chunk.fileName);
    for (const file of chunk.imports) closure(byFile.get(file), into);
    return into;
  };
  const entry = closure(chunks.find((c) => c.isEntry));
  const facade = (test: (id: string) => boolean) => chunks.find((c) => c.facadeModuleId !== null && test(c.facadeModuleId));
  const without = (files: Set<string>) => [...files].filter((f) => !entry.has(f));
  return {
    locale(locale) {
      const site = facade((id) => id === `${RESOLVED}site/${locale}`);
      if (!site) throw new Error(`kaeso-docs: the ${locale} site module is not in the bundle`);
      const files = closure(site);
      if (locale !== DEFAULT_LOCALE) {
        const ui = facade((id) => new RegExp(`/packages/ui/dist/chunks/${locale}-[^/]+\\.js$`).test(id));
        if (!ui) throw new Error(`kaeso-docs: @logicplanes/kaeso-ui's ${locale} messages are not in the bundle`);
        closure(ui, files);
      }
      return without(files);
    },
    page(path) {
      return without(closure(facade((id) => id === `${RESOLVED}${pageModuleId(path)}`)));
    },
  };
}

function writeHtml(outDir: string, file: string, html: string) {
  const target = join(outDir, file);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, html);
}

export function htmlFiles(path: string, isFolderIndex: boolean): string[] {
  if (path === "/") return ["index.html"];
  const base = path.slice(1);
  return isFolderIndex ? [`${base}.html`, `${base}/index.html`] : [`${base}.html`];
}

export function kaesoDocs(options: DocsOptions): Plugin {
  const contentDir = resolve(options.root, options.contentDir);
  let docs: Docs | null = null;
  let isBuild = false;
  const current = () => (docs ??= buildDocs(contentDir, options.root, options.version));
  const summary = (d: Docs) => `content: ${d.problems.length} to fix in ${options.contentDir}/\n  ${d.problems.join("\n  ")}`;
  const render = options.render ?? browserRender(options.root);
  const draw = async (d: Docs, prune: boolean): Promise<string[]> => {
    if (!missingDiagrams(d.diagramDir, d.diagrams).length && !prune) return [];
    try {
      const result = await syncDiagrams(d.diagramDir, d.diagrams, render, { prune });
      if (result.drawn || result.removed.length) docs = null;
      return result.problems;
    } catch (error) {
      const missing = new Set(missingDiagrams(d.diagramDir, d.diagrams).map((j) => j.key));
      const places = d.diagrams.filter((j) => missing.has(j.key)).map((j) => `${j.file}:${j.line}`);
      return [`can't draw ${places.join(", ")} (${(error as Error).message.split("\n")[0]}); run \`${DIAGRAM_COMMAND}\` where Chromium is installed`];
    }
  };

  return {
    name: "kaeso-docs",
    config(_config, env) {
      isBuild = env.command === "build";
    },
    async buildStart() {
      docs = null;
      if (isBuild) {
        const failed = await draw(current(), true);
        if (failed.length) this.error(`diagrams: ${failed.length} to fix\n  ${failed.join("\n  ")}`);
      }
      const d = current();
      for (const locale of LOCALES) for (const page of d.sites[locale].pages) this.addWatchFile(page.file);
      if (isBuild && d.problems.length) this.error(summary(d));
    },
    configureServer(server) {
      const invalidate = () => {
        for (const id of server.moduleGraph.idToModuleMap.keys()) {
          if (!id.startsWith(RESOLVED)) continue;
          const mod = server.moduleGraph.getModuleById(id);
          if (mod) server.moduleGraph.invalidateModule(mod);
        }
      };
      const report = () => {
        const d = current();
        if (missingDiagrams(d.diagramDir, d.diagrams).length) {
          void draw(d, false).then((failed) => {
            for (const problem of failed) server.config.logger.warn(problem);
            if (docs) return;
            invalidate();
            report();
            server.ws.send({ type: "full-reload" });
          });
          return;
        }
        if (d.problems.length) server.config.logger.warn(summary(d));
      };
      report();
      server.watcher.add(contentDir);
      const changed = (file: string) => {
        if (!resolve(file).startsWith(contentDir)) return;
        docs = null;
        invalidate();
        report();
        server.ws.send({ type: "full-reload" });
      };
      server.watcher.on("change", changed);
      server.watcher.on("add", changed);
      server.watcher.on("unlink", changed);
    },
    resolveId(id) {
      return id.startsWith(VIRTUAL) ? `${RESOLVED}${id.slice(VIRTUAL.length)}` : null;
    },
    load(id) {
      if (!id.startsWith(RESOLVED)) return null;
      const name = id.slice(RESOLVED.length);
      const d = current();
      const [kind = "", locale = ""] = name.split("/");
      if (kind === "page") return pageModule(d, name, options.root);
      if (!isLocale(locale)) return null;
      if (kind === "site") return siteModule(d.sites[locale], options.editBase);
      if (kind === "search") return `export default ${JSON.stringify(d.sites[locale].search)};\n`;
      return null;
    },
    writeBundle(output, bundle) {
      const d = current();
      const outDir = output.dir ?? join(options.root, "dist");
      const template = readFileSync(join(outDir, "index.html"), "utf8");
      const preload = preloads(bundle);
      for (const locale of LOCALES) {
        const siteName = MESSAGES[locale]["site.name"];
        const shared = preload.locale(locale);
        const head = (path: string | null, title: string, description: string, extra: string[]): PageHead => ({
          lang: locale,
          title: documentTitle(title, siteName),
          description,
          siteName,
          url: path === null ? "" : absoluteUrl(options.siteUrl, path),
          alternates: path === null ? [] : alternatesFor(options.siteUrl, path),
          preloads: [...new Set([...shared, ...extra])],
          image: absoluteUrl(options.siteUrl, OG_IMAGE.path),
          indexed: path !== null,
          structured: path === null ? null : structuredData(options.siteUrl, siteName, locale, { path, title, description }),
        });
        const site = d.sites[locale];
        for (const meta of site.metas) {
          const page = site.pages.find((p) => p.path === meta.path);
          const html = pageHtml(template, head(meta.path, meta.title, meta.description, preload.page(meta.path)));
          for (const file of htmlFiles(meta.path, page?.local.endsWith("index.md") ?? false)) writeHtml(outDir, file, html);
        }
        const folder = locale === DEFAULT_LOCALE ? "" : `${locale}/`;
        writeHtml(
          outDir,
          `${folder}404.html`,
          pageHtml(template, head(null, MESSAGES[locale]["notFound.title"], MESSAGES[locale]["notFound.description"], [])),
        );
      }
      writeFileSync(join(outDir, "sitemap.xml"), sitemap(d, options.siteUrl));
    },
  };
}
