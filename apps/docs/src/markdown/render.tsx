import { Fragment, useId, useRef, type ReactNode } from "react";
import { flushSync } from "react-dom";
import {
  Banner,
  CheckboxBox,
  Code,
  CodeBlock,
  Figure,
  KeyValue,
  Link,
  ScrollArea,
  Separator,
  Table,
  Text,
  ThemeImage,
  VisuallyHidden,
} from "@logicplanes/kaeso-ui";
import type { Locale } from "~/i18n/locales";
import { useSiteLocale, useText, type Translate } from "~/i18n/text";
import type { MessageKey } from "~/i18n/messages";
import type { Block, CalloutKind, CodeTab, Inline, LinkCard, ListItem } from "./ast";
import { chooseCodeTab, useCodeTab } from "./code-tab";

type Size = "body-lg" | "body";
interface Ctx {
  size: Size;
  tone: "default" | "secondary";
  t: Translate;
  locale: Locale;
  cell?: boolean;
  whole?: boolean;
  linkless?: boolean;
  tableLabel?: string;
}

const CALLOUT: Record<CalloutKind, { key: MessageKey; tone: "neutral" | "info" | "success" | "warning" }> = {
  note: { key: "callout.note", tone: "neutral" },
  tip: { key: "callout.tip", tone: "success" },
  important: { key: "callout.important", tone: "info" },
  warning: { key: "callout.warning", tone: "warning" },
  caution: { key: "callout.caution", tone: "warning" },
};

function plainText(xs: Inline[]): string {
  return xs.map((x) => (x.t === "text" || x.t === "code" ? x.v : x.t === "img" ? x.alt : x.t === "br" ? " " : "c" in x ? plainText(x.c) : "")).join("");
}

const LIST_SEPARATOR: Record<Locale, string> = { en: ", ", de: ", ", "zh-CN": "、" };
const SHORT_COLUMN = 20;
const MANY_COLUMNS = 5;
const NARROW_CELLS = 4;
const LONG_HEADER = 24;
const WHOLE_CODE = 16;
const WHOLE_NAME = 30;
const WIDE = /[\u2e80-\u9fff\uac00-\ud7af\uf900-\ufaff\ufe30-\ufe4f\uff00-\uffef]/u;

function width(s: string): number {
  let n = 0;
  for (const ch of s) n += WIDE.test(ch) ? 2 : 1;
  return n;
}

function shortColumns(rows: Inline[][][], columns: number): boolean[] {
  return Array.from({ length: columns }, (_, j) => rows.every((row) => width(plainText(row[j] ?? [])) <= SHORT_COLUMN));
}

const SOFT_BREAK = /(?<=\S[./_:,({[@=-]+)(?=[^\s./_:,(){}[\]@=-])|(?<=[a-z\d])(?=[A-Z][a-z])/;
const CJK_LIST_PUNCTUATION = /[、，；：]$/;

function codeText(v: string): ReactNode[] {
  return v.split(SOFT_BREAK).map((part, i) => (
    <Fragment key={i}>
      {i > 0 && <wbr />}
      {part}
    </Fragment>
  ));
}

function inline(xs: Inline[], ctx: Ctx): ReactNode[] {
  return xs.map((x, i) => {
    switch (x.t) {
      case "text":
        return (
          <Fragment key={i}>
            {x.v}
            {xs[i + 1]?.t === "code" && CJK_LIST_PUNCTUATION.test(x.v) && <wbr />}
          </Fragment>
        );
      case "strong":
        return (
          <Text key={i} as="strong" variant={ctx.size} weight="semibold" tone="inherit">
            {inline(x.c, ctx)}
          </Text>
        );
      case "em":
        return <em key={i}>{inline(x.c, ctx)}</em>;
      case "del":
        return <del key={i}>{inline(x.c, ctx)}</del>;
      case "code":
        return x.v.length <= (ctx.whole ? WHOLE_NAME : WHOLE_CODE) ? (
          <Code key={i} shape="rounded" flow={ctx.cell ? "cell" : "text"} className="whitespace-nowrap">
            {x.v}
          </Code>
        ) : (
          <Code key={i} shape="rounded" flow={ctx.cell ? "cell" : "text"} className={ctx.cell ? undefined : "wrap-anywhere"}>
            {codeText(x.v)}
          </Code>
        );
      case "br":
        return <br key={i} />;
      case "link": {
        if (ctx.linkless) return <Fragment key={i}>{inline(x.c, ctx)}</Fragment>;
        const leaves = x.kind === "external" || x.kind === "file";
        const bare = x.kind === "external" && plainText(x.c).replace(/\/$/, "") === x.href.replace(/^mailto:/, "").replace(/\/$/, "");
        return (
          <Link
            key={i}
            href={x.href}
            external={leaves && !bare}
            {...(x.title ? { title: x.title } : {})}
            className={`${leaves && !bare ? "max-w-full" : "inline"}${ctx.cell ? "" : " wrap-anywhere"}`}
          >
            {inline(x.c, ctx)}
          </Link>
        );
      }
      case "img":
        return (
          <img
            key={i}
            src={x.src}
            alt={x.alt}
            {...(x.title ? { title: x.title } : {})}
            {...(x.width && x.height ? { width: x.width, height: x.height } : {})}
            loading="lazy"
            decoding="async"
            className="inline-block max-w-full align-top"
          />
        );
    }
  });
}

const DIAGRAM_MIN_WIDTHS = [
  [320, "min-w-320"],
  [360, "min-w-360"],
  [440, "min-w-440"],
  [480, "min-w-480"],
  [560, "min-w-560"],
] as const;
const DIAGRAM_SHRINK = 0.75;

export function diagramMinWidth(width: number): string | undefined {
  const least = width * DIAGRAM_SHRINK;
  if (least < DIAGRAM_MIN_WIDTHS[0][0]) return undefined;
  return DIAGRAM_MIN_WIDTHS.find(([min]) => min >= least)?.[1] ?? "min-w-560";
}

const hasLink = (xs: Inline[]): boolean => xs.some((x) => x.t === "link" || ("c" in x && hasLink(x.c)));

function scrollParent(el: HTMLElement): Element | null {
  for (let parent = el.parentElement; parent; parent = parent.parentElement) {
    const { overflowY } = getComputedStyle(parent);
    if ((overflowY === "auto" || overflowY === "scroll") && parent.scrollHeight > parent.clientHeight) return parent;
  }
  return document.scrollingElement ?? document.documentElement;
}

function CodeGroup({ tabs }: { tabs: CodeTab[] }) {
  const chosen = useCodeTab();
  const ref = useRef<HTMLDivElement>(null);
  const choose = (label: string) => {
    const el = ref.current;
    const before = el?.getBoundingClientRect().top ?? 0;
    flushSync(() => chooseCodeTab(label));
    if (!el) return;
    const moved = el.getBoundingClientRect().top - before;
    if (moved !== 0) scrollParent(el)?.scrollBy(0, moved);
  };
  return <CodeBlock ref={ref} tabs={tabs} tab={chosen} onTabChange={choose} collapseAfter={0} wrap="phone" />;
}

function LinkCards({ items, ctx }: { items: LinkCard[]; ctx: Ctx }) {
  const text: Ctx = { ...ctx, size: "body", linkless: true };
  return (
    <ul className="grid gap-12 sm:grid-cols-2">
      {items.map((card, i) => (
        <li key={i} className="flex min-w-0">
          <Link href={card.href} variant="card" className="w-full min-w-0">
            <Text as="span" variant="body" weight="medium" tone="inherit" className="wrap-break-word">
              {inline(card.title, text)}
            </Text>
            {card.description.length > 0 && (
              <Text as="span" variant="small" tone="secondary" className="text-pretty wrap-break-word">
                {inline(card.description, text)}
              </Text>
            )}
          </Link>
        </li>
      ))}
    </ul>
  );
}

function Diagram({ b, ctx }: { b: Extract<Block, { t: "diagram" }>; ctx: Ctx }) {
  const id = useId();
  return (
    <Figure label={ctx.t("page.diagram").replace("{title}", b.title)} caption={b.title} focus={b.focus}>
      <ThemeImage
        light={b.light}
        dark={b.dark}
        alt={b.title}
        width={b.width}
        height={b.height}
        aria-describedby={b.description ? id : undefined}
        className={diagramMinWidth(b.width)}
      />
      {b.description && <VisuallyHidden id={id}>{b.description}</VisuallyHidden>}
    </Figure>
  );
}

const STACK_COLUMNS = 3;
const BESIDE_LABEL = 48;
const MOSTLY_SHORT = 0.75;

function StackedTable({ b, label, ctx }: { b: Extract<Block, { t: "table" }>; label: string; ctx: Ctx }) {
  const fits = (c: Inline[]) => width(plainText(c)) <= BESIDE_LABEL;
  const beside = Array.from({ length: b.head.length }, (_, j) => {
    const cells = b.rows.map((row) => row[j] ?? []).filter((c) => c.length > 0);
    return cells.filter(fits).length >= cells.length * MOSTLY_SHORT;
  });
  const pairs = (row: Inline[][], side: boolean) =>
    row.flatMap((c, j) =>
      j > 0 && c.length > 0 && (beside[j] === true && fits(c)) === side ? [{ label: inline(b.head[j] ?? [], ctx), value: inline(c, ctx), wrap: true }] : [],
    );
  return (
    <ul aria-label={label} className="flex flex-col gap-12 sm:hidden">
      {b.rows.map((row, r) => {
        const side = pairs(row, true);
        const below = pairs(row, false);
        return (
          <li key={r} className="flex flex-col gap-12">
            {r > 0 && <Separator />}
            <div className="flex flex-col gap-4">
              <Text as="div" variant="body" weight="medium" className="wrap-break-word">
                {inline(row[0] ?? [], ctx)}
              </Text>
              {side.length > 0 && <KeyValue items={side} />}
              {below.length > 0 && <KeyValue layout="stacked" items={below} />}
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function Item({ item, ordered, n, ctx }: { item: ListItem; ordered: boolean; n: number; ctx: Ctx }) {
  const first = item.c[0];
  const name = first && "c" in first && first.t === "paragraph" ? plainText(first.c) : ctx.t("page.task");
  return (
    <li className="flex gap-12">
      {item.task ? (
        <span className="flex shrink-0 items-center">
          <CheckboxBox checked={!!item.checked} readOnly aria-label={name} />
        </span>
      ) : (
        <Text
          as="span"
          aria-hidden
          variant={ctx.size}
          tone="secondary"
          tabular
          className={ordered ? "min-w-16 shrink-0 text-right" : "w-12 shrink-0 text-center"}
        >
          {ordered ? `${n}.` : "•"}
        </Text>
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-8">{blocks(item.c, ctx)}</div>
    </li>
  );
}

function block(b: Block, i: number, ctx: Ctx): ReactNode {
  switch (b.t) {
    case "heading": {
      const variant = b.level === 2 ? "title" : b.level === 3 ? "title-sm" : b.level === 4 ? "body-lg" : "body";
      const space = b.level === 2 ? "mt-32" : b.level === 3 ? "mt-16" : "mt-8";
      return (
        <Text
          key={i}
          as={`h${b.level}`}
          id={b.id}
          variant={variant}
          weight="semibold"
          className={`${space} scroll-mt-24 text-balance wrap-break-word first:mt-0`}
        >
          {hasLink(b.c) ? (
            inline(b.c, ctx)
          ) : (
            <Link href={`#${b.id}`} variant="heading">
              {inline(b.c, ctx)}
            </Link>
          )}
        </Text>
      );
    }
    case "paragraph":
      return (
        <Text key={i} as={b.tight ? "div" : "p"} variant={ctx.size} weight="regular" tone={ctx.tone} className="text-pretty wrap-break-word">
          {inline(b.c, ctx)}
        </Text>
      );
    case "list": {
      const Tag = b.ordered ? "ol" : "ul";
      return (
        <Tag key={i} className="flex flex-col gap-8" {...(b.ordered && b.start !== 1 ? { start: b.start } : {})}>
          {b.items.map((item, j) => (
            <Item key={j} item={item} ordered={b.ordered} n={b.start + j} ctx={ctx} />
          ))}
        </Tag>
      );
    }
    case "links":
      return <LinkCards key={i} items={b.items} ctx={ctx} />;
    case "codegroup":
      return <CodeGroup key={i} tabs={b.tabs} />;
    case "code":
      return <CodeBlock key={i} code={b.code} lang={b.lang} collapseAfter={0} wrap={b.wrap ?? "phone"} {...(b.title ? { title: b.title } : {})} />;
    case "quote":
      return (
        <blockquote key={i} className="flex gap-16">
          <Separator orientation="vertical" tone="default" />
          <div className="flex min-w-0 flex-1 flex-col gap-12">{blocks(b.c, { ...ctx, tone: "secondary" })}</div>
        </blockquote>
      );
    case "callout": {
      const c = CALLOUT[b.kind];
      return (
        <Banner key={i} tone={c.tone} title={ctx.t(c.key)}>
          {b.c.length > 0 && <div className="flex flex-col gap-8">{blocks(b.c, { ...ctx, size: "body", tone: "default" })}</div>}
        </Banner>
      );
    }
    case "table": {
      const cell: Ctx = { ...ctx, size: "body", tone: "default", cell: true };
      const short = shortColumns(b.rows, b.head.length);
      const columns = b.head.map(plainText).join(LIST_SEPARATOR[ctx.locale]);
      const stacked = b.head.length >= STACK_COLUMNS;
      const label = ctx.tableLabel ?? ctx.t("page.table").replace("{columns}", columns);
      return (
        <Fragment key={i}>
          <ScrollArea orientation="horizontal" aria-label={label} {...(stacked ? { className: "max-sm:hidden" } : {})}>
            <Table density={b.head.length >= MANY_COLUMNS ? "compact" : "default"}>
              <Table.Head>
                <Table.Row>
                  {b.head.map((c, j) => (
                    <Table.HeaderCell key={j} align={b.align[j] ?? "start"} wrap={width(plainText(c)) > LONG_HEADER}>
                      {inline(c, cell)}
                    </Table.HeaderCell>
                  ))}
                </Table.Row>
              </Table.Head>
              <Table.Body>
                {b.rows.map((row, r) => (
                  <Table.Row key={r}>
                    {row.map((c, j) => (
                      <Table.Cell
                        key={j}
                        align={b.align[j] ?? "start"}
                        className={`align-top ${short[j] ? "whitespace-nowrap" : b.head.length >= NARROW_CELLS ? "min-w-120" : "min-w-140"}`}
                      >
                        {inline(c, c.some((x) => x.t === "br") ? { ...cell, whole: true } : cell)}
                      </Table.Cell>
                    ))}
                  </Table.Row>
                ))}
              </Table.Body>
            </Table>
          </ScrollArea>
          {stacked && <StackedTable b={b} label={label} ctx={cell} />}
        </Fragment>
      );
    }
    case "diagram":
      return <Diagram key={i} b={b} ctx={ctx} />;
    case "rule":
      return <Separator key={i} className="my-8" />;
  }
}

function blocks(bs: Block[], ctx: Ctx): ReactNode[] {
  return bs.map((b, i) => block(b, i, ctx));
}

// Each table is a landmark named after its columns. Tables that share columns are named after
// their section as well (its heading under the page's last h2), then numbered if that isn't
// enough, so no two landmarks on a page share a name.
function tableLabels(bs: Block[], t: Translate, locale: Locale): Map<number, string> {
  const tables: { i: number; base: string; columns: string; section: string | undefined }[] = [];
  let h2: string | undefined;
  let section: string | undefined;
  bs.forEach((b, i) => {
    if (b.t === "heading") {
      const text = plainText(b.c);
      if (b.level === 2) h2 = text;
      section = b.level === 2 || !h2 ? text : `${h2} › ${text}`;
    } else if (b.t === "table") {
      const columns = b.head.map(plainText).join(LIST_SEPARATOR[locale]);
      tables.push({ i, columns, base: t("page.table").replace("{columns}", columns), section });
    }
  });
  const count = (xs: string[], x: string) => xs.filter((y) => y === x).length;
  const columns = tables.map((x) => x.columns);
  const named = tables.map((x) =>
    count(columns, x.columns) > 1 && x.section ? t("page.tableIn").replace("{table}", x.base).replace("{section}", x.section) : x.base,
  );
  const labels = new Map<number, string>();
  tables.forEach((x, n) => {
    const name = named[n] ?? x.base;
    const nth = named.slice(0, n + 1).filter((y) => y === name).length;
    const label = count(named, name) > 1 && nth > 1 ? `${name} (${nth})` : name;
    if (label !== x.base) labels.set(x.i, label);
  });
  return labels;
}

export function Markdown({ blocks: bs, className, lang }: { blocks: Block[]; className?: string; lang?: string | undefined }) {
  const t = useText();
  const locale = useSiteLocale();
  const labels = tableLabels(bs, t, locale);
  return (
    <div lang={lang} className={`flex min-w-0 flex-col gap-16${className ? ` ${className}` : ""}`}>
      {bs.map((b, i) => {
        const tableLabel = labels.get(i);
        return block(b, i, { size: "body-lg", tone: "default", t, locale, ...(tableLabel ? { tableLabel } : {}) });
      })}
    </div>
  );
}
