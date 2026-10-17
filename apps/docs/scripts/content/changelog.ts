import { existsSync, readFileSync } from "node:fs";
import { relative, sep } from "node:path";
import { Lexer, type Token, type Tokens } from "marked";
import { history, type ChangelogEntry as Written } from "../../../../scripts/changelog.mjs";
import type { Messages } from "../../src/i18n/messages/index.ts";
import type { Block, ChangelogContent, ChangelogEntry, ChangelogGroup, NavGroup, PageMeta, SearchEntry, TocEntry } from "../../src/markdown/ast.ts";
import { compile, lex, type Resolver } from "./compile.ts";
import type { Site, SitePage } from "./site.ts";

export interface Changelog {
  file: string;
  rel: string;
  content: ChangelogContent;
  words: string[];
  problems: string[];
}

const GROUPS: Readonly<Record<string, ChangelogGroup>> = {
  Added: "added",
  Changed: "changed",
  Deprecated: "deprecated",
  Removed: "removed",
  Fixed: "fixed",
  Security: "security",
};

const rawHtmlOffsets = (markdown: string): number[] => {
  const found: number[] = [];
  const visit = (tokens: Token[], raw: string, base: number) => {
    let cursor = 0;
    for (const token of tokens) {
      const at = raw.indexOf(token.raw, cursor);
      const offset = at === -1 ? base : base + at;
      if (at !== -1) cursor = at + token.raw.length;
      if (token.type === "html") found.push(offset);
      const cells = token.type === "table" ? [...(token as Tokens.Table).header, ...(token as Tokens.Table).rows.flat()] : [];
      const children = [
        ...("tokens" in token && token.tokens ? token.tokens : []),
        ...(token.type === "list" ? (token as Tokens.List).items : []),
        ...cells.flatMap((cell) => cell.tokens),
      ];
      visit(children, token.raw, offset);
    }
  };
  visit(new Lexer({ gfm: true }).lex(markdown), markdown, 0);
  return found;
};

const posix = (p: string) => p.split(sep).join("/");

export function buildChangelog(file: string, repoRoot: string): Changelog {
  const rel = posix(relative(repoRoot, file));
  if (!existsSync(file)) return { file, rel, content: { releases: [] }, words: [], problems: [`${rel} does not exist`] };
  const markdown = readFileSync(file, "utf8");
  const log = history(markdown);
  const written = [...(log.unreleased ? [log.unreleased] : []), ...log.releases];
  const anchors = new Set(written.flatMap((e) => [e.id, ...e.sections.flatMap((s) => [s.id, ...s.topics.map((t) => t.id)])]));
  const problems: string[] = [];
  const words = new Set<string>();
  const resolver: Resolver = {
    anchors,
    resolve: (href) => ({
      kind: "problem",
      message: `links to ${href}: link to a full https address, or to a version on this page by its anchor, v and the version`,
    }),
  };
  const rawHtml = (source: string, line: number, limit: number) => {
    const lines = rawHtmlOffsets(source)
      .filter((offset) => offset < limit)
      .map((offset) => line + source.slice(0, offset).split("\n").length - 1);
    for (const at of new Set(lines)) problems.push(`${rel}: line ${at}: raw HTML is not allowed; write it in a code span or remove it`);
  };
  const fragment = (markdown: string, where: string, line: number): Block[] => {
    if (!markdown) return [];
    const source = `${markdown}\n\n${log.definitions}`;
    rawHtml(source, line, markdown.length);
    const compiled = compile(lex(source, { references: true }), resolver);
    for (const problem of compiled.problems) problems.push(`${rel}: [${where}] ${problem}`);
    for (const word of compiled.words) words.add(word);
    return compiled.blocks;
  };
  const entry = (e: Written, where: string): ChangelogEntry => ({
    id: e.id,
    intro: fragment(e.intro, where, e.introLine),
    sections: e.sections.map((s) => {
      const group = GROUPS[s.name];
      return {
        name: s.name,
        ...(group ? { group } : {}),
        id: s.id,
        blocks: fragment(s.markdown, where, s.line),
        topics: s.topics.map((t) => ({ name: t.name, id: t.id, blocks: fragment(t.markdown, where, t.line) })),
      };
    }),
  });
  const optional = <K extends string>(key: K, value: string | null) => (value ? ({ [key]: value } as Record<K, string>) : {});
  const content: ChangelogContent = {
    releases: log.releases.map((r) => ({
      ...entry(r, r.version),
      version: r.version,
      ...optional("date", r.date),
      prerelease: r.prerelease,
      yanked: r.yanked,
      latest: r.latest,
      ...optional("previous", r.previous),
      ...optional("url", r.url),
      ...optional("compare", r.compare),
    })),
    ...(log.unreleased
      ? { unreleased: { ...entry(log.unreleased, "Unreleased"), ...optional("since", log.unreleased.since), ...optional("compare", log.unreleased.compare) } }
      : {}),
    ...optional("repository", log.repository),
  };
  return { file, rel, content, words: [...words], problems };
}

export function changelogToc(content: ChangelogContent, messages: Messages): TocEntry[] {
  return [
    ...(content.unreleased ? [{ id: content.unreleased.id, label: messages["changelog.unreleased"], depth: 0 as const }] : []),
    ...content.releases.map((r) => ({ id: r.id, label: r.version, depth: 0 as const })),
  ];
}

function withItem(nav: NavGroup[], item: NavGroup["items"][number]): NavGroup[] {
  const [first, ...rest] = nav;
  if (first?.id !== "pages") return [{ id: "pages", items: [item] }, ...nav];
  return [{ ...first, items: [...first.items.slice(0, 1), item, ...first.items.slice(1)] }, ...rest];
}

function relinked(metas: PageMeta[]): PageMeta[] {
  return metas.map((m, i) => {
    const prev = metas[i - 1];
    const next = metas[i + 1];
    return {
      path: m.path,
      title: m.title,
      description: m.description,
      label: m.label,
      file: m.file,
      ...(prev ? { prev: { path: prev.path, title: prev.title } } : {}),
      ...(next ? { next: { path: next.path, title: next.title } } : {}),
    };
  });
}

const changelogPath = (prefix: string) => `${prefix}/changelog`;

export function changelogClash(site: Site, prefix: string): string[] {
  const path = changelogPath(prefix);
  return site.pages.filter((p) => p.path === path).map((p) => `${p.rel}: its address ${path} is the changelog's; rename it`);
}

export function withChangelog(site: Site, changelog: Changelog, prefix: string, messages: Messages): Site {
  const path = changelogPath(prefix);
  const title = messages["changelog.title"];
  const description = messages["changelog.description"];
  const toc = changelogToc(changelog.content, messages);
  const page: SitePage = {
    file: changelog.file,
    rel: changelog.rel,
    local: changelog.rel,
    path,
    meta: { title, description },
    label: title,
    lexed: { tokens: [], headings: [] },
    compiled: { blocks: [], toc, words: changelog.words, problems: [], changelog: changelog.content },
  };
  const nav = withItem(site.nav, { path, label: title, depth: 0 });
  const order = nav.flatMap((g) => g.items.map((i) => i.path));
  const metas = new Map(site.metas.map((m) => [m.path, m]));
  metas.set(path, { path, title, description, label: title, file: changelog.rel });
  const entry: SearchEntry = {
    path,
    title,
    description,
    headings: toc.map(({ id, label }) => ({ id, label })),
    words: changelog.words.join(" ").slice(0, 6000),
  };
  return {
    ...site,
    pages: [...site.pages, page],
    nav,
    metas: relinked(order.flatMap((p) => metas.get(p) ?? [])),
    search: [...site.search, entry],
  };
}
