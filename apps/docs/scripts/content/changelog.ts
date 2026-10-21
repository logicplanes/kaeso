import { existsSync, readFileSync } from "node:fs";
import { relative, sep } from "node:path";
import { Lexer, type Token, type Tokens } from "marked";
import { history, type ChangelogEntry as Written } from "../../../../scripts/changelog.mjs";
import type { Messages } from "../../src/i18n/messages/index.ts";
import type {
  Block,
  ChangelogContent,
  ChangelogEntry,
  ChangelogGroup,
  ChangelogRelease,
  Inline,
  NavGroup,
  PageMeta,
  SearchEntry,
  TocEntry,
} from "../../src/markdown/ast.ts";
import { compile, lex, type Resolver } from "./compile.ts";
import type { Site, SitePage } from "./site.ts";

export interface Changelog {
  file: string;
  rel: string;
  content: ChangelogContent;
  words: string[];
  entryWords: Record<string, string[]>;
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
  if (!existsSync(file)) return { file, rel, content: { releases: [] }, words: [], entryWords: {}, problems: [`${rel} does not exist`] };
  const markdown = readFileSync(file, "utf8");
  const log = history(markdown);
  const written = [...(log.unreleased ? [log.unreleased] : []), ...log.releases];
  const anchors = new Set(written.flatMap((e) => [e.id, ...e.sections.flatMap((s) => [s.id, ...s.topics.map((t) => t.id)])]));
  const problems: string[] = [];
  const words = new Set<string>();
  const entryWords: Record<string, string[]> = {};
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
  const fragment = (markdown: string, where: string, line: number, into: Set<string>): Block[] => {
    if (!markdown) return [];
    const source = `${markdown}\n\n${log.definitions}`;
    rawHtml(source, line, markdown.length);
    const compiled = compile(lex(source, { references: true }), resolver);
    for (const problem of compiled.problems) problems.push(`${rel}: [${where}] ${problem}`);
    for (const word of compiled.words) {
      words.add(word);
      into.add(word);
    }
    return compiled.blocks;
  };
  const entry = (e: Written, where: string): ChangelogEntry => {
    const own = new Set<string>();
    const compiled = {
      id: e.id,
      intro: fragment(e.intro, where, e.introLine, own),
      sections: e.sections.map((s) => {
        const group = GROUPS[s.name];
        return {
          name: s.name,
          ...(group ? { group } : {}),
          id: s.id,
          blocks: fragment(s.markdown, where, s.line, own),
          topics: s.topics.map((t) => ({ name: t.name, id: t.id, blocks: fragment(t.markdown, where, t.line, own) })),
        };
      }),
    };
    entryWords[e.id] = [...own];
    return compiled;
  };
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
  return { file, rel, content, words: [...words], entryWords, problems };
}

export interface ChangelogPage {
  path: string;
  series?: string;
  content: ChangelogContent;
}

const seriesOf = (version: string) => (version.split("-")[0] ?? version).split(".").slice(0, 2).join(".");

const newerOrSame = (series: string, than: string) => {
  const [a = 0, b = 0] = series.split(".").map(Number);
  const [c = 0, d = 0] = than.split(".").map(Number);
  return a !== c ? a > c : b >= d;
};

const idsOf = (e: ChangelogEntry) => [e.id, ...e.sections.flatMap((s) => [s.id, ...s.topics.map((t) => t.id)])];

type Away = (id: string) => string | undefined;

function relinkInlines(inlines: Inline[], away: Away): Inline[] {
  return inlines.map((x) => {
    if (x.t === "link") {
      const page = x.kind === "anchor" ? away(x.href.slice(1)) : undefined;
      return { ...x, ...(page ? { kind: "page" as const, href: `${page}${x.href}` } : {}), c: relinkInlines(x.c, away) };
    }
    return x.t === "strong" || x.t === "em" || x.t === "del" ? { ...x, c: relinkInlines(x.c, away) } : x;
  });
}

function relinkBlocks(blocks: Block[], away: Away): Block[] {
  return blocks.map((b): Block => {
    switch (b.t) {
      case "heading":
      case "paragraph":
        return { ...b, c: relinkInlines(b.c, away) };
      case "list":
        return { ...b, items: b.items.map((item) => ({ ...item, c: relinkBlocks(item.c, away) })) };
      case "quote":
      case "callout":
        return { ...b, c: relinkBlocks(b.c, away) };
      case "table":
        return { ...b, head: b.head.map((cell) => relinkInlines(cell, away)), rows: b.rows.map((row) => row.map((cell) => relinkInlines(cell, away))) };
      case "links":
        return {
          ...b,
          items: b.items.map((card) => {
            const page = card.kind === "anchor" ? away(card.href.slice(1)) : undefined;
            return {
              ...card,
              ...(page ? { kind: "page" as const, href: `${page}${card.href}` } : {}),
              title: relinkInlines(card.title, away),
              description: relinkInlines(card.description, away),
            };
          }),
        };
      default:
        return b;
    }
  });
}

function relinkEntry<E extends ChangelogEntry>(e: E, away: Away): E {
  return {
    ...e,
    intro: relinkBlocks(e.intro, away),
    sections: e.sections.map((s) => ({
      ...s,
      blocks: relinkBlocks(s.blocks, away),
      topics: s.topics.map((t) => ({ ...t, blocks: relinkBlocks(t.blocks, away) })),
    })),
  };
}

// One page per minor series keeps every page under the docs' page budget (scripts/budget.ts) as releases add up.
export function changelogPages(content: ChangelogContent, base: string): ChangelogPage[] {
  const newest = content.releases.find((r) => r.latest) ?? content.releases[0];
  const current = newest && seriesOf(newest.version);
  const groups = new Map<string | undefined, ChangelogRelease[]>([[undefined, []]]);
  for (const r of content.releases) {
    const series = seriesOf(r.version);
    const key = current === undefined || newerOrSame(series, current) ? undefined : series;
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  const pathOf = (series: string | undefined) => (series ? `${base}/${series}` : base);
  const where = new Map<string, string>(content.unreleased ? idsOf(content.unreleased).map((id) => [id, base]) : []);
  for (const [series, releases] of groups) for (const r of releases) for (const id of idsOf(r)) where.set(id, pathOf(series));
  const pages = [...groups.keys()].map((series) => ({ path: pathOf(series), ...(series ? { series } : {}) }));
  return [...groups].map(([series, releases]) => {
    const path = pathOf(series);
    const away: Away = (id) => {
      const page = where.get(id);
      return page && page !== path ? page : undefined;
    };
    const page: ChangelogContent = {
      releases: releases.map((r) => relinkEntry(r, away)),
      ...(!series && content.unreleased ? { unreleased: relinkEntry(content.unreleased, away) } : {}),
      ...(content.repository ? { repository: content.repository } : {}),
      ...(pages.length > 1 ? { pages, elsewhere: Object.fromEntries([...where].filter(([, p]) => p !== path)) } : {}),
    };
    return { path, ...(series ? { series } : {}), content: page };
  });
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
  const base = changelogPath(prefix);
  return site.pages.filter((p) => p.path === base || p.path.startsWith(`${base}/`)).map((p) => `${p.rel}: its address ${p.path} is the changelog's; rename it`);
}

export function withChangelog(site: Site, changelog: Changelog, prefix: string, messages: Messages): Site {
  const base = changelogPath(prefix);
  const pages = changelogPages(changelog.content, base).map((p) => {
    const title = p.series ? messages["changelog.seriesTitle"].replace("{series}", p.series) : messages["changelog.title"];
    const description = p.series ? messages["changelog.seriesDescription"].replace("{series}", p.series) : messages["changelog.description"];
    const entries = [...(p.content.unreleased ? [p.content.unreleased] : []), ...p.content.releases];
    const words = [...new Set(entries.flatMap((e) => changelog.entryWords[e.id] ?? []))];
    return { ...p, title, description, words, toc: changelogToc(p.content, messages) };
  });
  const sitePages: SitePage[] = pages.map((p) => ({
    file: changelog.file,
    rel: changelog.rel,
    local: changelog.rel,
    path: p.path,
    meta: { title: p.title, description: p.description },
    label: p.title,
    lexed: { tokens: [], headings: [] },
    compiled: { blocks: [], toc: p.toc, words: p.words, problems: [], changelog: p.content },
  }));
  const title = messages["changelog.title"];
  const nav = withItem(site.nav, { path: base, label: title, depth: 0 });
  const order = nav.flatMap((g) => g.items.map((i) => i.path));
  const metas = new Map(site.metas.map((m) => [m.path, m]));
  metas.set(base, { path: base, title, description: messages["changelog.description"], label: title, file: changelog.rel });
  const older: PageMeta[] = pages
    .filter((p) => p.series)
    .map((p) => ({ path: p.path, title: p.title, description: p.description, label: p.title, file: changelog.rel }));
  const search: SearchEntry[] = pages.map((p) => ({
    path: p.path,
    title: p.title,
    description: p.description,
    headings: p.toc.map(({ id, label }) => ({ id, label })),
    words: p.words.join(" ").slice(0, 6000),
  }));
  return {
    ...site,
    pages: [...site.pages, ...sitePages],
    nav,
    metas: [...relinked(order.flatMap((p) => metas.get(p) ?? [])), ...older],
    search: [...site.search, ...search],
  };
}
