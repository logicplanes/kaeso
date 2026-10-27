import { forwardRef, memo, useMemo, useState, type ReactNode } from "react";
import { Lexer, Tokenizer, type Token, type Tokens } from "marked";
import { cn } from "../../utils/cn.js";
import { safeHref } from "../../utils/safe-href.js";
import { useUiText } from "../../i18n/context.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { Button } from "../button/button.js";
import { Code } from "../code/code.js";
import { CodeBlock } from "../code-block/code-block.js";
import type { CodeLang } from "../code-block/highlighter.js";
import { Link } from "../link/link.js";
import { Separator } from "../separator/separator.js";
import { Table } from "../table/table.js";
import { Text } from "../text/text.js";

export { safeHref };

export const KAESO_MARKDOWN_VARIANTS = {
  size: {
    lg: { classes: "gap-16", description: "15/22 text, 16px between blocks: a model's response, for reading" },
    body: { classes: "gap-12", description: "14/20 text, 12px between blocks: responses and prompts (default)" },
    small: { classes: "gap-8", description: "13/18 text, 8px between blocks: reasoning, tool results, side notes" },
  },
  tone: {
    default: { classes: "text-fg", description: "Primary text (default)" },
    secondary: { classes: "text-fg-secondary", description: "Quieter text: reasoning summaries and quotes" },
  },
} as const;
export const KAESO_MARKDOWN_DEFAULT_VARIANTS = { size: "body", tone: "default" } as const;

export type MarkdownSize = keyof typeof KAESO_MARKDOWN_VARIANTS.size;
export type MarkdownTone = keyof typeof KAESO_MARKDOWN_VARIANTS.tone;

export interface MarkdownProps {
  text: string;
  size?: MarkdownSize | undefined;
  tone?: MarkdownTone | undefined;
  headingLevel?: 1 | 2 | 3 | 4 | 5 | 6 | undefined;
  collapseAfter?: number | undefined;
  streaming?: boolean | undefined;
  wrapCode?: boolean | undefined;
  trailing?: ReactNode | undefined;
  className?: string | undefined;
}

interface Ctx {
  wrap: boolean;
  size: MarkdownSize;
  level: number;
  trailing: ReactNode;
}

function lexer() {
  const tokenizer = new Tokenizer();
  tokenizer.html = () => undefined;
  tokenizer.tag = () => undefined;
  tokenizer.def = () => undefined;
  return new Lexer({ gfm: true, breaks: true, tokenizer });
}

let cache: Map<string, Token[]> | undefined;

export function lexMarkdown(text: string, options: { cache?: boolean | undefined } = {}): Token[] {
  if (options.cache === false) return lexer().lex(text);
  cache ??= new Map();
  const hit = cache.get(text);
  if (hit) return hit;
  const tokens = lexer().lex(text);
  if (cache.size >= 64) cache.delete(cache.keys().next().value ?? "");
  cache.set(text, tokens);
  return tokens;
}

export function previewMarkdown(text: string, limit: number): string {
  if (text.length <= limit) return text;
  for (const at of ["\n\n", "\n"]) {
    const cut = text.lastIndexOf(at, limit);
    if (cut > limit / 2) return text.slice(0, cut);
  }
  const space = text.lastIndexOf(" ", limit);
  return `${text.slice(0, space > limit / 2 ? space : limit).trimEnd()}…`;
}

export function markdownCodeLang(lang: string | undefined): CodeLang {
  const name = (lang ?? "").trim().split(/\s/)[0]?.toLowerCase() ?? "";
  switch (name) {
    case "ts":
    case "typescript":
      return "ts";
    case "tsx":
    case "jsx":
      return "tsx";
    case "js":
    case "javascript":
    case "mjs":
    case "cjs":
      return "js";
    case "json":
    case "jsonc":
    case "json5":
      return "json";
    case "bash":
    case "sh":
    case "shell":
    case "zsh":
    case "console":
      return "bash";
    case "python":
    case "py":
      return "python";
    case "yaml":
    case "yml":
      return "yaml";
    case "diff":
    case "patch":
      return "diff";
    default:
      return "text";
  }
}

function ExternalLink({ href, title, children }: { href: string; title?: string | null | undefined; children: ReactNode }) {
  return (
    <Link href={href} external target="_blank" rel="noopener noreferrer nofollow" {...(title ? { title } : {})} className="max-w-full wrap-anywhere">
      {children}
    </Link>
  );
}

function inline(tokens: Token[] | undefined): ReactNode[] {
  return (tokens ?? []).map((t, i) => {
    switch (t.type) {
      case "text":
        return "tokens" in t && t.tokens ? <span key={i}>{inline(t.tokens)}</span> : t.text;
      case "escape":
        return t.text;
      case "strong":
        return (
          <strong key={i} className="font-semibold">
            {inline(t.tokens)}
          </strong>
        );
      case "em":
        return <em key={i}>{inline(t.tokens)}</em>;
      case "del":
        return <del key={i}>{inline(t.tokens)}</del>;
      case "codespan":
        return (
          <Code key={i} flow="text" className="wrap-anywhere">
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
            {inline(link.tokens)}
          </ExternalLink>
        ) : (
          <span key={i}>{inline(link.tokens)}</span>
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

function Task({ done }: { done: boolean }) {
  const t = useUiText();
  return (
    <span
      role="img"
      aria-label={done ? t("markdown.taskDone", "Done") : t("markdown.taskOpen", "Not done")}
      className={cn("mt-3 inline-flex size-14 shrink-0 items-center justify-center rounded-checkbox", done ? "bg-ink text-on-ink" : "shadow-ring")}
    >
      {done && <Icon icon={icons.check} size="xs" className="size-12" />}
    </span>
  );
}

function List({ list, ctx }: { list: Tokens.List; ctx: Ctx }) {
  const Tag = list.ordered ? "ol" : "ul";
  const start = typeof list.start === "number" ? list.start : 1;
  const last = list.items.length - 1;
  const digits = String(start + last).length;
  return (
    <Tag className="flex flex-col gap-4">
      {list.items.map((item, i) => (
        <li key={i} className="flex gap-8">
          {item.task ? (
            <Task done={!!item.checked} />
          ) : (
            <Text
              as="span"
              aria-hidden
              variant={TEXT[ctx.size]}
              weight="regular"
              tone="tertiary"
              tabular
              className={list.ordered ? "shrink-0 text-right" : "w-8 shrink-0 text-center"}
              {...(list.ordered ? { style: { minWidth: `${digits + 0.5}ch` } } : {})}
            >
              {list.ordered ? `${start + i}.` : "•"}
            </Text>
          )}
          <div className="flex min-w-0 flex-1 flex-col gap-4">
            {blocks(
              item.tokens.filter((t) => t.type !== "checkbox"),
              i === last ? ctx : { ...ctx, trailing: null },
              !item.loose,
            )}
          </div>
        </li>
      ))}
    </Tag>
  );
}

const PROSE = "max-w-prose wrap-break-word";

const TEXT = { lg: "body-lg", body: "body", small: "small" } as const;
const HEADING = {
  lg: ["title", "title-sm", "body-lg", "body-lg"],
  body: ["title-sm", "body-lg", "body", "body"],
  small: ["body-lg", "body", "small", "small"],
} as const;

function block(t: Token, ctx: Ctx, tight: boolean): ReactNode {
  const tail = ctx.trailing;
  switch (t.type) {
    case "space":
      return null;
    case "heading": {
      const h = t as Tokens.Heading;
      const step = Math.min(3, h.depth - 1);
      const variant = HEADING[ctx.size][step]!;
      const level = Math.min(6, h.depth + ctx.level - 1) as 1 | 2 | 3 | 4 | 5 | 6;
      return (
        <Text as={`h${level}`} variant={variant} weight={step === 3 ? "medium" : "semibold"} tone="inherit" className="mt-4 first:mt-0 wrap-break-word">
          {inline(h.tokens)}
          {tail}
        </Text>
      );
    }
    case "paragraph":
      return (
        <Text as="p" variant={TEXT[ctx.size]} weight="regular" tone="inherit" className={PROSE}>
          {inline((t as Tokens.Paragraph).tokens)}
          {tail}
        </Text>
      );
    case "text": {
      const x = t as Tokens.Text;
      return (
        <Text as={tight ? "div" : "p"} variant={TEXT[ctx.size]} weight="regular" tone="inherit" className={PROSE}>
          {x.tokens ? inline(x.tokens) : x.text}
          {tail}
        </Text>
      );
    }
    case "list":
      return <List list={t as Tokens.List} ctx={ctx} />;
    case "code":
      return (
        <>
          <CodeBlock code={(t as Tokens.Code).text} lang={markdownCodeLang((t as Tokens.Code).lang)} wrap={ctx.wrap} />
        </>
      );
    case "blockquote":
      return (
        <blockquote className="flex gap-12">
          <Separator orientation="vertical" tone="default" />
          <div className="flex min-w-0 flex-1 flex-col gap-8 text-fg-secondary">{blocks((t as Tokens.Blockquote).tokens, ctx)}</div>
        </blockquote>
      );
    case "hr":
      return (
        <>
          <Separator className="my-4" />
        </>
      );
    case "table": {
      const table = t as Tokens.Table;
      const align = (a: Tokens.TableCell["align"]) => (a === "right" ? "end" : "start");
      return (
        <Table className={ctx.size === "small" ? "text-small" : undefined}>
          <Table.Head>
            <Table.Row>
              {table.header.map((c, j) => (
                <Table.HeaderCell key={j} align={align(c.align)}>
                  {inline(c.tokens)}
                </Table.HeaderCell>
              ))}
            </Table.Row>
          </Table.Head>
          <Table.Body>
            {table.rows.map((row, r) => (
              <Table.Row key={r}>
                {row.map((c, j) => (
                  <Table.Cell key={j} align={align(c.align)} className={c.text.length <= 24 ? "whitespace-nowrap" : undefined}>
                    {inline(c.tokens)}
                  </Table.Cell>
                ))}
              </Table.Row>
            ))}
          </Table.Body>
        </Table>
      );
    }
    default:
      return (
        <Text as="p" variant={TEXT[ctx.size]} weight="regular" tone="inherit" className={cn(PROSE, "whitespace-pre-wrap")}>
          {t.raw}
          {tail}
        </Text>
      );
  }
}

const Block = memo(
  function Block({ token, ctx, tight }: { token: Token; ctx: Ctx; tight: boolean }) {
    return <>{block(token, ctx, tight)}</>;
  },
  (a, b) =>
    a.token.raw === b.token.raw &&
    a.tight === b.tight &&
    a.ctx.size === b.ctx.size &&
    a.ctx.wrap === b.ctx.wrap &&
    a.ctx.level === b.ctx.level &&
    a.ctx.trailing === b.ctx.trailing,
);

function blocks(tokens: Token[], ctx: Ctx, tight = false): ReactNode[] {
  let last = tokens.length - 1;
  while (last > 0 && tokens[last]?.type === "space") last--;
  return tokens.map((t, i) => <Block key={i} token={t} tight={tight} ctx={i === last || !ctx.trailing ? ctx : { ...ctx, trailing: null }} />);
}

export const Markdown = forwardRef<HTMLDivElement, MarkdownProps>(function Markdown(
  { text, size = "body", tone = "default", headingLevel = 3, collapseAfter = 24_000, streaming = false, wrapCode = false, trailing, className },
  ref,
) {
  const [all, setAll] = useState(false);
  const t = useUiText();
  const long = !streaming && collapseAfter > 0 && text.length > collapseAfter;
  const shown = long && !all ? previewMarkdown(text, Math.round(collapseAfter * 0.66)) : text;
  const tokens = useMemo(() => {
    const out = lexMarkdown(shown, { cache: !streaming });
    let end = out.length;
    if (shown !== text) while (end > 1 && ["space", "heading", "hr"].includes(out[end - 1]?.type ?? "")) end--;
    return out.slice(0, end);
  }, [shown, text, streaming]);
  const ctx = useMemo<Ctx>(() => ({ size, wrap: wrapCode, level: headingLevel, trailing: trailing ?? null }), [size, wrapCode, headingLevel, trailing]);
  return (
    <div ref={ref} className={cn("flex min-w-0 flex-col", KAESO_MARKDOWN_VARIANTS.size[size].classes, KAESO_MARKDOWN_VARIANTS.tone[tone].classes, className)}>
      {blocks(tokens, ctx)}
      {long && !all && (
        <div>
          <Button size="sm" variant="secondary" onClick={() => setAll(true)}>
            {t("markdown.showAllCharacters", "Show all {count} characters", { count: text.length })}
          </Button>
        </div>
      )}
    </div>
  );
});
Markdown.displayName = "Markdown";
