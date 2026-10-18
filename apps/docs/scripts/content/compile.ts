import { Lexer, Tokenizer, type Token, type Tokens } from "marked";
import type { Block, CalloutKind, ChangelogContent, CodeLang, Inline, LinkCard, ListItem, TocEntry } from "../../src/markdown/ast.ts";
import { DIAGRAM_LANG, diagramText } from "./diagram.ts";
import { Slugger } from "./slug.ts";

interface Heading {
  level: number;
  id: string;
  text: string;
  nested?: boolean;
}

export interface Lexed {
  tokens: Token[];
  headings: Heading[];
  h1?: string;
  source?: string;
  offset?: number;
}

export type Target = { kind: "page"; href: string } | { kind: "file"; ref: string; width?: number; height?: number } | { kind: "problem"; message: string };

export type DiagramTarget =
  { kind: "diagram"; light: string; dark: string; width: number; height: number; labels: string[] } | { kind: "problem"; message: string };

export interface Resolver {
  resolve(href: string, as: "link" | "image"): Target;
  diagram?(code: string, line: number): DiagramTarget;
  anchors: Set<string>;
}

function lexer(references = false) {
  const tokenizer = new Tokenizer();
  tokenizer.html = () => undefined;
  tokenizer.tag = () => undefined;
  if (!references) tokenizer.def = () => undefined;
  return new Lexer({ gfm: true, breaks: false, tokenizer });
}

function plain(tokens: Token[] | undefined): string {
  return (tokens ?? [])
    .map((t) => {
      if ("tokens" in t && t.tokens && t.type !== "codespan") return plain(t.tokens);
      if (t.type === "codespan" || t.type === "text" || t.type === "escape") return (t as Tokens.Text).text;
      if (t.type === "image") return (t as Tokens.Image).text;
      if (t.type === "br") return " ";
      return t.raw;
    })
    .join("");
}

export function lex(body: string, options: { references?: boolean; offset?: number } = {}): Lexed {
  const tokens = lexer(options.references).lex(body);
  const slugger = new Slugger();
  const headings: Heading[] = [];
  let h1: string | undefined;
  const first = tokens.find((t) => t.type !== "space");
  const walk = (ts: Token[], nested: boolean) => {
    for (const t of ts) {
      if (t.type === "heading") {
        const h = t as Tokens.Heading;
        const text = plain(h.tokens).trim();
        if (h.depth === 1 && t === first) h1 = text;
        else headings.push({ level: h.depth, id: slugger.slug(text), text, ...(nested ? { nested } : {}) });
      } else if (t.type === "blockquote") walk((t as Tokens.Blockquote).tokens, true);
      else if (t.type === "list") for (const item of (t as Tokens.List).items) walk(item.tokens, true);
    }
  };
  walk(tokens, false);
  return { tokens, headings, ...(h1 !== undefined ? { h1 } : {}), source: body, offset: options.offset ?? 0 };
}

const LANGS: Record<string, CodeLang> = {
  ts: "ts",
  typescript: "ts",
  tsx: "tsx",
  jsx: "tsx",
  js: "js",
  javascript: "js",
  mjs: "js",
  cjs: "js",
  json: "json",
  jsonc: "json",
  json5: "json",
  bash: "bash",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  console: "bash",
  shellsession: "bash",
  python: "python",
  py: "python",
  yaml: "yaml",
  yml: "yaml",
  diff: "diff",
  patch: "diff",
};

const DRAWING = /[\u2190-\u21ff\u2500-\u257f\u25b2-\u25c5]/u;
const isDiagram = (info: string | undefined) => (info ?? "").trim().split(/\s+/)[0]?.toLowerCase() === DIAGRAM_LANG;

function attribute(raw: string, name: string): string | undefined {
  const match = new RegExp(`\\b${name}=(?:"([^"]*)"|'([^']*)'|(\\S+))`).exec(raw);
  return (match?.[1] ?? match?.[2] ?? match?.[3])?.trim() || undefined;
}

export function fenceInfo(info: string | undefined): { lang: CodeLang; title?: string; tab?: string } {
  const raw = (info ?? "").trim();
  const name = raw.split(/\s+/)[0]?.toLowerCase() ?? "";
  const title = attribute(raw, "title");
  const tab = attribute(raw, "tab");
  return { lang: LANGS[name] ?? "text", ...(title ? { title } : {}), ...(tab ? { tab } : {}) };
}

export function safeExternal(href: string): string | null {
  try {
    const url = new URL(href.trim());
    return url.protocol === "http:" || url.protocol === "https:" || url.protocol === "mailto:" ? url.href : null;
  } catch {
    return null;
  }
}

const SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const compactHref = (href: string) => [...href].filter((ch) => ch.charCodeAt(0) > 0x20 && ch.charCodeAt(0) !== 0x7f).join("");
const CALLOUT = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\][ \t]*(?:\n|$)/i;

interface Ctx {
  resolver: Resolver;
  problems: string[];
  source: string;
  offset: number;
  cursor: number;
  labels: Map<Block, string[]>;
}

function fenceLine(code: Tokens.Code, ctx: Ctx): number {
  const first = code.text.split("\n").find((l) => l.trim()) ?? "";
  const fence = ctx.source.indexOf(code.raw.trimStart().split("\n")[0] ?? "", ctx.cursor);
  const from = fence === -1 ? ctx.cursor : fence;
  const at = first ? ctx.source.indexOf(first.trim(), from) : -1;
  const start = fence !== -1 ? fence : at === -1 ? ctx.cursor : ctx.source.lastIndexOf("\n", at) + 1;
  ctx.cursor = Math.max(ctx.cursor, start + 1);
  return ctx.offset + ctx.source.slice(0, start).split("\n").length;
}

function diagram(code: Tokens.Code, ctx: Ctx): Block | null {
  const line = fenceLine(code, ctx);
  const at = (message: string) => ctx.problems.push(`:${line}: ${message}`);
  if (!ctx.resolver.diagram) {
    at("diagrams are drawn only on the pages in content/; show this as text or a list");
    return null;
  }
  if (/^[ \t]*%%/m.test(code.text)) at("the diagram has a `%%` line: diagrams take no comments and no `%%{init}`, the site sets their look");
  const text = diagramText(code.text);
  if (!text.title) at("the diagram has no `accTitle: …`: name it in a few words for screen readers and search");
  const target = ctx.resolver.diagram(code.text, line);
  if (target.kind === "problem") {
    at(target.message);
    return null;
  }
  if (!text.title) return null;
  const block: Block = {
    t: "diagram",
    light: target.light,
    dark: target.dark,
    width: target.width,
    height: target.height,
    title: text.title,
    ...(text.description ? { description: text.description } : {}),
  };
  ctx.labels.set(block, target.labels);
  return block;
}

function linkTarget(href: string, ctx: Ctx): { kind: "page" | "anchor" | "external" | "file"; href: string } | null {
  const h = href.trim();
  const compact = compactHref(h);
  if (!h) {
    ctx.problems.push("a link has no address");
    return null;
  }
  if (h.startsWith("#")) {
    const id = decodeURIComponent(h.slice(1));
    if (!ctx.resolver.anchors.has(id)) {
      ctx.problems.push(`links to #${id}, which is not a heading on this page`);
      return null;
    }
    return { kind: "anchor", href: `#${id}` };
  }
  if (SCHEME.test(compact) || compact.startsWith("//")) {
    const safe = safeExternal(compact.startsWith("//") ? "" : h);
    if (!safe) {
      ctx.problems.push(`links to "${h.slice(0, 60)}": only http, https and mailto addresses, and relative links to files in content/, are allowed`);
      return null;
    }
    return { kind: "external", href: safe };
  }
  const target = ctx.resolver.resolve(h, "link");
  if (target.kind === "problem") {
    ctx.problems.push(target.message);
    return null;
  }
  if (target.kind === "page") return { kind: "page", href: target.href };
  return { kind: "file", href: target.ref };
}

function inline(tokens: Token[] | undefined, ctx: Ctx): Inline[] {
  const out: Inline[] = [];
  const text = (v: string) => {
    const last = out[out.length - 1];
    if (last?.t === "text") last.v += v;
    else if (v) out.push({ t: "text", v });
  };
  const add = (xs: Inline[]) => xs.forEach((x) => (x.t === "text" ? text(x.v) : out.push(x)));
  for (const t of tokens ?? []) {
    switch (t.type) {
      case "text":
        if ("tokens" in t && t.tokens?.length) add(inline(t.tokens, ctx));
        else text((t as Tokens.Text).text);
        break;
      case "escape":
        text((t as Tokens.Escape).text);
        break;
      case "strong":
      case "em":
      case "del":
        out.push({ t: t.type, c: inline((t as Tokens.Strong).tokens, ctx) });
        break;
      case "codespan":
        out.push({ t: "code", v: (t as Tokens.Codespan).text });
        break;
      case "br":
        out.push({ t: "br" });
        break;
      case "link": {
        const link = t as Tokens.Link;
        const children = inline(link.tokens, ctx);
        const target = linkTarget(link.href, ctx);
        if (!target) add(children);
        else out.push({ t: "link", kind: target.kind, href: target.href, ...(link.title ? { title: link.title } : {}), c: children });
        break;
      }
      case "image": {
        const image = t as Tokens.Image;
        const h = image.href.trim();
        if (SCHEME.test(compactHref(h)) || h.startsWith("//")) {
          ctx.problems.push(`shows the image "${h.slice(0, 60)}" from outside content/: put the file next to the page and link it relatively`);
          text(image.text);
          break;
        }
        const target = ctx.resolver.resolve(h, "image");
        if (target.kind !== "file") {
          ctx.problems.push(target.kind === "problem" ? target.message : `shows "${h}", which is a page, not an image`);
          text(image.text);
          break;
        }
        if (!image.text.trim()) ctx.problems.push(`the image ${h} has no alt text: describe it in the brackets, ![like this](${h})`);
        out.push({
          t: "img",
          src: target.ref,
          alt: image.text,
          ...(image.title ? { title: image.title } : {}),
          ...(target.width && target.height ? { width: target.width, height: target.height } : {}),
        });
        break;
      }
      case "checkbox":
        break;
      default:
        text(t.raw);
    }
  }
  return out;
}

const CARD_SEPARATOR = /^\s*(?:[:：]|—|–)\s*/u;

function linkCards(items: ListItem[]): LinkCard[] | null {
  const cards: LinkCard[] = [];
  for (const item of items) {
    const only = item.c.length === 1 ? item.c[0] : undefined;
    if (item.task || only?.t !== "paragraph") return null;
    const [link, ...rest] = only.c;
    if (link?.t !== "link" || (link.kind !== "page" && link.kind !== "anchor")) return null;
    const [first, ...after] = rest;
    const lead = first?.t === "text" ? first.v.replace(CARD_SEPARATOR, "") : null;
    const description: Inline[] = lead === null ? rest : lead ? [{ t: "text", v: lead }, ...after] : after;
    cards.push({ href: link.href, kind: link.kind, title: link.c, description });
  }
  return cards.length > 0 ? cards : null;
}

function blocks(tokens: Token[], ctx: Ctx, state: { headings: Heading[]; next: number; tight?: boolean }): Block[] {
  const out: Block[] = [];
  const groups: { block: Extract<Block, { t: "codegroup" }>; line: number }[] = [];
  let group: (typeof groups)[number] | null = null;
  for (const t of tokens) {
    if (t.type !== "space" && t.type !== "code") group = null;
    switch (t.type) {
      case "space":
      case "def":
        break;
      case "heading": {
        const h = t as Tokens.Heading;
        const heading = state.headings[state.next++];
        if (!heading) break;
        if (h.depth === 1) {
          ctx.problems.push(`has a \`# ${heading.text}\` heading below the top: the page's title comes from its frontmatter, so its headings start at \`##\``);
          break;
        }
        out.push({ t: "heading", level: h.depth as 2 | 3 | 4 | 5 | 6, id: heading.id, c: inline(h.tokens, ctx) });
        break;
      }
      case "paragraph":
        out.push({ t: "paragraph", c: inline((t as Tokens.Paragraph).tokens, ctx) });
        break;
      case "text": {
        const x = t as Tokens.Text;
        out.push({ t: "paragraph", c: x.tokens ? inline(x.tokens, ctx) : [{ t: "text", v: x.text }], ...(state.tight ? { tight: true } : {}) });
        break;
      }
      case "list": {
        const list = t as Tokens.List;
        const items: ListItem[] = list.items.map((item) => ({
          ...(item.task ? { task: true, checked: !!item.checked } : {}),
          c: blocks(
            item.tokens.filter((x) => x.type !== "checkbox"),
            ctx,
            { ...state, tight: !item.loose },
          ),
        }));
        const cards = list.ordered ? null : linkCards(items);
        out.push(cards ? { t: "links", items: cards } : { t: "list", ordered: list.ordered, start: typeof list.start === "number" ? list.start : 1, items });
        break;
      }
      case "code": {
        const code = t as Tokens.Code;
        if (isDiagram(code.lang)) {
          group = null;
          const block = diagram(code, ctx);
          if (block) out.push(block);
          break;
        }
        const line = fenceLine(code, ctx);
        if (DRAWING.test(code.text))
          ctx.problems.push(
            `:${line}: the code block draws a diagram with box-drawing characters or arrows; write it as a \`mermaid\` block (development/documentation.md, Diagrams)`,
          );
        const { tab, ...info } = fenceInfo(code.lang);
        if (!tab) {
          group = null;
          out.push({ t: "code", code: code.text, ...info });
          break;
        }
        if (info.title) ctx.problems.push(`:${line}: the fence has both \`tab="${tab}"\` and \`title=\`; a fence in a code group is named by its tab alone`);
        if (group?.block.tabs.some((x) => x.label === tab))
          ctx.problems.push(`:${line}: the code group has two tabs named "${tab}"; give each fence its own \`tab=\``);
        const entry = { label: tab, lang: info.lang, code: code.text };
        if (group) group.block.tabs.push(entry);
        else {
          group = { block: { t: "codegroup", tabs: [entry] }, line };
          groups.push(group);
          out.push(group.block);
        }
        break;
      }
      case "blockquote": {
        const quote = t as Tokens.Blockquote;
        const callout = CALLOUT.exec(quote.text);
        if (callout) {
          const kind = (callout[1] ?? "note").toLowerCase() as CalloutKind;
          const rest = lexer().lex(quote.text.slice(callout[0].length));
          out.push({ t: "callout", kind, c: blocks(rest, ctx, state) });
        } else {
          out.push({ t: "quote", c: blocks(quote.tokens, ctx, state) });
        }
        break;
      }
      case "hr":
        out.push({ t: "rule" });
        break;
      case "table": {
        const table = t as Tokens.Table;
        out.push({
          t: "table",
          align: table.align.map((a) => (a === "right" ? "end" : "start")),
          head: table.header.map((c) => inline(c.tokens, ctx)),
          rows: table.rows.map((row) => row.map((c) => inline(c.tokens, ctx))),
        });
        break;
      }
      default:
        if (t.raw.trim()) out.push({ t: "paragraph", c: [{ t: "text", v: t.raw.trim() }] });
    }
  }
  for (const { block, line } of groups) {
    const [only] = block.tabs;
    if (block.tabs.length > 1 || !only) continue;
    ctx.problems.push(
      `:${line}: the fence with \`tab="${only.label}"\` stands alone; a code group is two or more fences with \`tab=\` in a row, and a single block has no \`tab=\``,
    );
    out[out.indexOf(block)] = { t: "code", lang: only.lang, code: only.code };
  }
  return out;
}

export interface Compiled {
  blocks: Block[];
  toc: TocEntry[];
  words: string[];
  problems: string[];
  changelog?: ChangelogContent;
}

function words(bs: Block[], into: Set<string>, labels: Map<Block, string[]> = new Map()): void {
  const add = (s: string) => {
    for (const w of s.toLowerCase().split(/[^\p{L}\p{N}_]+/u)) if (w.length >= 2) into.add(w);
  };
  const inl = (xs: Inline[]) => {
    for (const x of xs) {
      if (x.t === "text" || x.t === "code") add(x.v);
      else if (x.t === "img") add(x.alt);
      else if ("c" in x) inl(x.c);
    }
  };
  for (const b of bs) {
    if (b.t === "heading" || b.t === "paragraph") inl(b.c);
    else if (b.t === "list") for (const i of b.items) words(i.c, into, labels);
    else if (b.t === "quote" || b.t === "callout") words(b.c, into, labels);
    else if (b.t === "diagram") [b.title, b.description ?? "", ...(labels.get(b) ?? [])].forEach(add);
    else if (b.t === "table") [...b.head, ...b.rows.flat()].forEach(inl);
    else if (b.t === "code" && b.code.length < 2000) add(b.code);
    else if (b.t === "codegroup") b.tabs.filter((tab) => tab.code.length < 2000).forEach((tab) => add(tab.code));
    else if (b.t === "links") b.items.forEach((card) => inl([...card.title, ...card.description]));
  }
}

export function compile(page: Lexed, resolver: Resolver): Compiled {
  const ctx: Ctx = { resolver, problems: [], source: page.source ?? "", offset: page.offset ?? 0, cursor: 0, labels: new Map() };
  const first = page.tokens.find((t) => t.type !== "space");
  const tokens = page.h1 !== undefined ? page.tokens.filter((t) => t !== first) : page.tokens;
  const out = blocks(tokens, ctx, { headings: page.headings, next: 0 });
  const toc: TocEntry[] = page.headings
    .filter((h) => !h.nested && (h.level === 2 || h.level === 3))
    .map((h) => ({ id: h.id, label: h.text, depth: h.level === 2 ? 0 : 1 }));
  const set = new Set<string>();
  words(out, set, ctx.labels);
  return { blocks: out, toc, words: [...set], problems: ctx.problems };
}
