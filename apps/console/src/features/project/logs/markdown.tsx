import { useMemo, useState, type ReactNode } from "react";
import { Lexer, Tokenizer, type Token, type Tokens } from "marked";
import { Button, Code, CodeBlock, Link, Separator, Table, Text, type CodeLang } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type MarkdownSize = "body" | "small";
type Tone = "default" | "secondary";
interface Ctx {
  size: MarkdownSize;
  tone: Tone;
}

export const LONG_TEXT = 24_000;
const PREVIEW = 16_000;

function lexer() {
  const tokenizer = new Tokenizer();
  tokenizer.html = () => undefined;
  tokenizer.tag = () => undefined;
  tokenizer.def = () => undefined;
  return new Lexer({ gfm: true, breaks: true, tokenizer });
}

const cache = new Map<string, Token[]>();
export function lex(text: string): Token[] {
  const hit = cache.get(text);
  if (hit) return hit;
  const tokens = lexer().lex(text);
  if (cache.size >= 64) cache.delete(cache.keys().next().value ?? "");
  cache.set(text, tokens);
  return tokens;
}

function preview(text: string): string {
  const cut = text.lastIndexOf("\n\n", PREVIEW);
  return text.slice(0, cut > PREVIEW / 2 ? cut : PREVIEW);
}

export function safeHref(href: string): string | null {
  try {
    const url = new URL(href.trim());
    return url.protocol === "http:" || url.protocol === "https:" || url.protocol === "mailto:" ? url.href : null;
  } catch {
    return null;
  }
}

const LANGS: Record<string, CodeLang> = {
  ts: "ts",
  typescript: "ts",
  tsx: "tsx",
  js: "js",
  javascript: "js",
  jsx: "tsx",
  mjs: "js",
  json: "json",
  jsonc: "json",
  bash: "bash",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  console: "bash",
  python: "python",
  py: "python",
  yaml: "yaml",
  yml: "yaml",
  diff: "diff",
};
const codeLang = (lang: string | undefined): CodeLang =>
  LANGS[
    (lang ?? "")
      .trim()
      .replace(/\s[\s\S]*$/, "")
      .toLowerCase()
  ] ?? "text";

function ExternalLink({ href, title, children }: { href: string; title?: string | null | undefined; children: ReactNode }) {
  return (
    <Link href={href} external target="_blank" rel="noopener noreferrer nofollow" {...(title ? { title } : {})} className="max-w-full wrap-anywhere">
      {children}
    </Link>
  );
}

function inline(tokens: Token[] | undefined, ctx: Ctx): ReactNode[] {
  return (tokens ?? []).map((t, i) => {
    switch (t.type) {
      case "text":
      case "escape":
        return t.text;
      case "strong":
        return (
          <Text key={i} as="strong" variant={ctx.size} tone="inherit" weight="semibold">
            {inline(t.tokens, ctx)}
          </Text>
        );
      case "em":
        return <em key={i}>{inline(t.tokens, ctx)}</em>;
      case "del":
        return <del key={i}>{inline(t.tokens, ctx)}</del>;
      case "codespan":
        return (
          <Code key={i} shape="rounded" className="wrap-anywhere">
            {t.text}
          </Code>
        );
      case "br":
        return <br key={i} />;
      case "link": {
        const link = t as Tokens.Link;
        const href = safeHref(link.href);
        return href ? (
          <ExternalLink key={i} href={href} title={link.title}>
            {inline(link.tokens, ctx)}
          </ExternalLink>
        ) : (
          <span key={i}>{inline(link.tokens, ctx)}</span>
        );
      }
      case "image": {
        const image = t as Tokens.Image;
        const href = safeHref(image.href);
        const label = image.text || image.href;
        return href ? (
          <ExternalLink key={i} href={href} title={image.title}>
            {label}
          </ExternalLink>
        ) : (
          label
        );
      }
      case "checkbox":
        return null;
      default:
        return t.raw;
    }
  });
}

function List({ list, ctx }: { list: Tokens.List; ctx: Ctx }) {
  const Tag = list.ordered ? "ol" : "ul";
  const start = typeof list.start === "number" ? list.start : 1;
  return (
    <Tag className="flex flex-col gap-4">
      {list.items.map((item, i) => (
        <li key={i} className="flex gap-8">
          {item.task ? (
            <Text as="span" variant={ctx.size} tone="tertiary" mono className="shrink-0">
              {item.checked ? "[x]" : "[ ]"}
            </Text>
          ) : (
            <Text
              as="span"
              aria-hidden
              variant={ctx.size}
              tone="tertiary"
              tabular
              className={list.ordered ? "min-w-16 shrink-0 text-right" : "w-8 shrink-0 text-center"}
            >
              {list.ordered ? `${start + i}.` : "•"}
            </Text>
          )}
          <div className="flex min-w-0 flex-1 flex-col gap-4">
            {blocks(
              item.tokens.filter((t) => t.type !== "checkbox"),
              ctx,
            )}
          </div>
        </li>
      ))}
    </Tag>
  );
}

function block(t: Token, i: number, ctx: Ctx): ReactNode {
  switch (t.type) {
    case "space":
      return null;
    case "heading": {
      const h = t as Tokens.Heading;
      const major = h.depth <= 2;
      const variant = ctx.size === "body" ? (major ? "body-lg" : "body") : major ? "body" : "small";
      const level = Math.min(6, h.depth + 2) as 3 | 4 | 5 | 6;
      return (
        <Text key={i} as={`h${level}`} variant={variant} weight="semibold" tone={ctx.tone} className="mt-8 first:mt-0 wrap-break-word text-balance">
          {inline(h.tokens, ctx)}
        </Text>
      );
    }
    case "paragraph":
      return (
        <Text key={i} as="p" variant={ctx.size} tone={ctx.tone} className="max-w-prose wrap-break-word text-pretty">
          {inline((t as Tokens.Paragraph).tokens, ctx)}
        </Text>
      );
    case "text":
      return (
        <Text key={i} as="div" variant={ctx.size} tone={ctx.tone} className="max-w-prose wrap-break-word text-pretty">
          {inline((t as Tokens.Text).tokens, ctx)}
        </Text>
      );
    case "list":
      return <List key={i} list={t as Tokens.List} ctx={ctx} />;
    case "code":
      return <CodeBlock key={i} code={(t as Tokens.Code).text} lang={codeLang((t as Tokens.Code).lang)} />;
    case "blockquote":
      return (
        <blockquote key={i} className="flex gap-12">
          <Separator orientation="vertical" tone="default" />
          <div className="flex min-w-0 flex-1 flex-col gap-8">{blocks((t as Tokens.Blockquote).tokens, { ...ctx, tone: "secondary" })}</div>
        </blockquote>
      );
    case "hr":
      return <Separator key={i} className="my-4" />;
    case "table": {
      const table = t as Tokens.Table;
      const align = (a: Tokens.TableCell["align"]) => (a === "right" ? "end" : "start");
      return (
        <div key={i} className="max-w-full overflow-x-auto">
          <Table>
            <Table.Head>
              <Table.Row>
                {table.header.map((c, j) => (
                  <Table.HeaderCell key={j} align={align(c.align)}>
                    {inline(c.tokens, ctx)}
                  </Table.HeaderCell>
                ))}
              </Table.Row>
            </Table.Head>
            <Table.Body>
              {table.rows.map((row, r) => (
                <Table.Row key={r}>
                  {row.map((c, j) => (
                    <Table.Cell key={j} align={align(c.align)}>
                      {inline(c.tokens, ctx)}
                    </Table.Cell>
                  ))}
                </Table.Row>
              ))}
            </Table.Body>
          </Table>
        </div>
      );
    }
    default:
      return (
        <Text key={i} as="p" variant={ctx.size} tone={ctx.tone} className="max-w-prose whitespace-pre-wrap wrap-break-word">
          {t.raw}
        </Text>
      );
  }
}

function blocks(tokens: Token[], ctx: Ctx): ReactNode[] {
  return tokens.map((t, i) => block(t, i, ctx));
}

export function Markdown({ text, size = "body", tone = "default" }: { text: string; size?: MarkdownSize; tone?: Tone }) {
  const [all, setAll] = useState(false);
  const long = text.length > LONG_TEXT;
  const shown = long && !all ? preview(text) : text;
  const rendered = useMemo(() => {
    const tokens = lex(shown);
    const end = shown === text ? tokens.length : Math.max(1, tokens.findLastIndex((tok) => !["space", "heading", "hr"].includes(tok.type)) + 1);
    return blocks(tokens.slice(0, end), { size, tone });
  }, [shown, text, size, tone]);
  return (
    <div className="flex min-w-0 flex-col gap-8">
      {rendered}
      {long && !all && (
        <div>
          <Button size="sm" variant="secondary" onClick={() => setAll(true)}>
            {t("logs.markdown.showAll", "{count, plural, one {Show all # character} other {Show all # characters}}", { count: text.length })}
          </Button>
        </div>
      )}
    </div>
  );
}
