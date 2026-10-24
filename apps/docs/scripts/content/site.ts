import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { imageSize } from "image-size";
import type { NavGroup, NavItem, PageMeta, SearchEntry } from "../../src/markdown/ast.ts";
import { compile, lex, type Compiled, type DiagramTarget, type Lexed, type Resolver, type Target } from "./compile.ts";
import { DIAGRAM_COMMAND, DIAGRAM_THEMES, diagramFile, diagramKey, mermaidVersion, svgLabels, svgFocus, svgSize, type DiagramJob } from "./diagram.ts";
import { parseFrontmatter, type Frontmatter } from "./frontmatter.ts";
import { humanize, pathSegment } from "./slug.ts";

const IMAGE_TYPES = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".svg"]);

export interface SiteOptions {
  prefix: string;
  labels: { home: string; overview: string };
  version: string;
  diagrams?: string;
}

export const located = (file: string, problem: string) => `${file}${problem.startsWith(":") ? "" : ": "}${problem}`;

export interface SitePage {
  file: string;
  rel: string;
  local: string;
  path: string;
  meta: Frontmatter;
  label: string;
  lexed: Lexed;
  compiled?: Compiled;
}

export interface Site {
  contentDir: string;
  pages: SitePage[];
  nav: NavGroup[];
  metas: PageMeta[];
  search: SearchEntry[];
  assets: Set<string>;
  diagrams: DiagramJob[];
  problems: string[];
}

interface Folder {
  dir: string;
  segments: string[];
  level: number;
}

interface Entry {
  page: SitePage;
  name: string;
  children: Entry[];
}

interface Section {
  name: string;
  index?: SitePage | undefined;
  entries: Entry[];
}

const hidden = (name: string) => name.startsWith("_") || name.startsWith(".");
const posix = (p: string) => p.split(sep).join("/");
const byOrder = (a: { page: SitePage; name: string }, b: { page: SitePage; name: string }) =>
  (a.page.meta.order ?? Infinity) - (b.page.meta.order ?? Infinity) || a.name.localeCompare(b.name);

const VERSION_TOKEN = "{{version}}";

const withVersion = (source: string, version: string) => source.replace(/(\\?)\{\{version\}\}/g, (_, escaped: string) => (escaped ? VERSION_TOKEN : version));

export function buildSite(contentDir: string, repoRoot: string, options: SiteOptions): Site {
  const problems: string[] = [];
  const pages: SitePage[] = [];
  const byFile = new Map<string, SitePage>();
  const byPath = new Map<string, SitePage>();
  const rel = (f: string) => posix(relative(repoRoot, f));
  const address = (segments: string[]) => (segments.length ? `${options.prefix}/${segments.join("/")}` : options.prefix || "/");

  if (!existsSync(contentDir))
    return { contentDir, pages, nav: [], metas: [], search: [], assets: new Set(), diagrams: [], problems: [`${rel(contentDir)} does not exist`] };

  const read = (file: string, segments: string[]) => {
    const parsed = parseFrontmatter(withVersion(readFileSync(file, "utf8"), options.version));
    const name = file.split(sep).pop() ?? "";
    const isIndex = name.toLowerCase() === "index.md";
    const path = address([...segments, ...(isIndex ? [] : [pathSegment(name)])]);
    const label = parsed.data.nav ?? (isIndex && segments.length < 2 ? (segments.length ? options.labels.overview : options.labels.home) : parsed.data.title);
    const page: SitePage = {
      file,
      rel: rel(file),
      local: posix(relative(contentDir, file)),
      path,
      meta: parsed.data,
      label,
      lexed: lex(parsed.body, { offset: parsed.lines }),
    };
    for (const p of parsed.problems) problems.push(`${page.rel}: ${p}`);
    if (page.lexed.h1 !== undefined && page.lexed.h1 !== page.meta.title)
      problems.push(`${page.rel}: starts with \`# ${page.lexed.h1}\`, but its title is "${page.meta.title}"; make them the same or leave the heading out`);
    const clash = byPath.get(path);
    if (clash) problems.push(`${page.rel}: has the same address as ${clash.rel} (${path}); rename one`);
    if (!pathSegment(name) && !isIndex) problems.push(`${page.rel}: its name has no letters or digits for an address`);
    pages.push(page);
    byFile.set(file, page);
    byPath.set(path, page);
    return page;
  };

  const sections: Section[] = [];
  const walk = (folder: Folder): { index?: SitePage; entries: Entry[] } => {
    const names = readdirSync(folder.dir)
      .filter((n) => !hidden(n))
      .sort();
    let index: SitePage | undefined;
    const entries: Entry[] = [];
    for (const name of names) {
      const full = join(folder.dir, name);
      if (statSync(full).isDirectory()) {
        const segment = pathSegment(name);
        if (!segment) {
          problems.push(`${rel(full)}: its name has no letters or digits for an address`);
          continue;
        }
        const sub = walk({ dir: full, segments: [...folder.segments, segment], level: folder.level + 1 });
        if (!sub.index && !sub.entries.length) continue;
        if (folder.level === 0) sections.push({ name, index: sub.index, entries: sub.entries });
        else if (folder.level >= 3) problems.push(`${rel(full)}: folders go at most three levels deep in ${rel(contentDir)}/`);
        else if (!sub.index) problems.push(`${rel(full)}: a folder inside a section needs an index.md, which is its page in the sidebar`);
        else entries.push({ page: sub.index, name, children: sub.entries });
      } else if (name.toLowerCase().endsWith(".md")) {
        const page = read(full, folder.segments);
        if (name.toLowerCase() === "index.md") index = page;
        else entries.push({ page, name, children: [] });
      }
    }
    entries.sort(byOrder);
    return { ...(index ? { index } : {}), entries };
  };
  const top = walk({ dir: contentDir, segments: [], level: 0 });
  if (!top.index) problems.push(`${rel(contentDir)}/index.md is missing: it is the home page`);

  const flatten = (entries: Entry[], depth: 0 | 1 | 2): NavItem[] =>
    entries.flatMap((e) => [{ path: e.page.path, label: e.page.label, depth }, ...flatten(e.children, depth === 0 ? 1 : 2)]);
  const nav: NavGroup[] = [];
  const topItems = [...(top.index ? [{ path: top.index.path, label: top.index.label, depth: 0 as const }] : []), ...flatten(top.entries, 0)];
  if (topItems.length) nav.push({ id: "pages", items: topItems });
  const sectionOf = new Map<string, string>();
  for (const s of sections
    .map((s) => ({ ...s, label: s.index?.meta.title ?? humanize(s.name), order: s.index?.meta.order ?? Infinity }))
    .sort((a, b) => a.order - b.order || a.name.localeCompare(b.name))) {
    const items = [...(s.index ? [{ path: s.index.path, label: s.index.label, depth: 0 as const }] : []), ...flatten(s.entries, 0)];
    for (const i of items) sectionOf.set(i.path, s.label);
    nav.push({ id: pathSegment(s.name), label: s.label, items });
  }

  const assets = new Set<string>();
  const diagrams: DiagramJob[] = [];
  const diagramDir = options.diagrams ?? resolve(contentDir, "..", "..", "diagrams");
  const renderer = mermaidVersion(repoRoot);
  const diagram = (page: SitePage, code: string, line: number): DiagramTarget => {
    const key = diagramKey(code, renderer);
    diagrams.push({ key, code, file: page.rel, line });
    const [light, dark] = DIAGRAM_THEMES.map((theme) => join(diagramDir, diagramFile(key, theme)));
    if (!light || !dark || !existsSync(light) || !existsSync(dark))
      return { kind: "problem", message: `the diagram has changed since it was last drawn; run \`${DIAGRAM_COMMAND}\` and commit ${rel(diagramDir)}/` };
    const svg = readFileSync(light, "utf8");
    const size = svgSize(svg);
    if (!size) return { kind: "problem", message: `${rel(light)} has no width and height; run \`${DIAGRAM_COMMAND}\` again` };
    const focus = svgFocus(svg);
    return { kind: "diagram", light: rel(light), dark: rel(dark), ...size, ...(focus === undefined ? {} : { focus }), labels: svgLabels(svg) };
  };
  const resolver = (page: SitePage): Resolver => ({
    anchors: new Set(page.lexed.headings.map((h) => h.id)),
    diagram: (code, line) => diagram(page, code, line),
    resolve(href: string, as: "link" | "image"): Target {
      const [pathPart = "", ...hashParts] = href.split("#");
      const hash = hashParts.join("#");
      let target: string;
      try {
        target = decodeURI(pathPart.split("?")[0] ?? "");
      } catch {
        return { kind: "problem", message: `has a link with a malformed address: ${href}` };
      }
      const abs = target.startsWith("/") ? join(repoRoot, target) : resolve(dirname(page.file), target);
      const shown = target || href;
      if (abs !== contentDir && !abs.startsWith(contentDir + sep))
        return { kind: "problem", message: `links to ${shown}, which is outside ${rel(contentDir)}/` };
      if (!existsSync(abs)) return { kind: "problem", message: `links to ${shown}, which doesn't exist` };
      const isDir = statSync(abs).isDirectory();
      const md = isDir ? join(abs, "index.md") : abs;
      if (as === "link" && (isDir || abs.toLowerCase().endsWith(".md"))) {
        const linked = byFile.get(md);
        if (!linked) return { kind: "problem", message: `links to ${shown}, which is not a page of the site` };
        if (!hash) return { kind: "page", href: linked.path };
        const id = decodeURIComponent(hash);
        if (!linked.lexed.headings.some((h) => h.id === id))
          return { kind: "problem", message: `links to ${shown}#${hash}, but ${linked.rel} has no heading with that anchor` };
        return { kind: "page", href: `${linked.path}#${id}` };
      }
      if (isDir) return { kind: "problem", message: `links to the folder ${shown}, which has no index.md` };
      const ext = extname(abs).toLowerCase();
      if (as === "image" && !IMAGE_TYPES.has(ext))
        return { kind: "problem", message: `shows ${shown}, which is not an image (${[...IMAGE_TYPES].join(", ")})` };
      const ref = rel(abs);
      assets.add(ref);
      if (!IMAGE_TYPES.has(ext)) return { kind: "file", ref };
      try {
        const size = imageSize(readFileSync(abs));
        return { kind: "file", ref, ...(size.width && size.height ? { width: size.width, height: size.height } : {}) };
      } catch {
        return { kind: "problem", message: `shows ${shown}, which can't be read as an image` };
      }
    },
  });
  for (const page of pages) {
    page.compiled = compile(page.lexed, resolver(page));
    for (const p of page.compiled.problems) problems.push(located(page.rel, p));
  }

  const order = nav.flatMap((g) => g.items.map((i) => i.path));
  const ordered = order.flatMap((path) => byPath.get(path) ?? []);
  const metas: PageMeta[] = ordered.map((page, i) => {
    const prev = ordered[i - 1];
    const next = ordered[i + 1];
    return {
      path: page.path,
      title: page.meta.title,
      description: page.meta.description,
      label: page.label,
      file: page.rel,
      ...(prev ? { prev: { path: prev.path, title: prev.meta.title } } : {}),
      ...(next ? { next: { path: next.path, title: next.meta.title } } : {}),
    };
  });

  const search: SearchEntry[] = ordered.map((page) => {
    const section = sectionOf.get(page.path);
    return {
      path: page.path,
      title: page.meta.title,
      ...(section ? { section } : {}),
      description: page.meta.description,
      headings: (page.compiled?.toc ?? []).map((t) => ({ id: t.id, label: t.label })),
      words: (page.compiled?.words ?? []).join(" ").slice(0, 6000),
    };
  });

  return { contentDir, pages, nav, metas, search, assets, diagrams, problems };
}
