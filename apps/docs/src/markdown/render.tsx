import { Fragment, type ReactNode } from "react";
import { Banner, Checkbox, Code, CodeBlock, Link, ScrollArea, Separator, Table, Text } from "@logicplanes/kaeso-ui";
import type { Locale } from "~/i18n/locales";
import { useSiteLocale, useText, type Translate } from "~/i18n/text";
import type { MessageKey } from "~/i18n/messages";
import type { Block, CalloutKind, Inline, ListItem } from "./ast";

type Size = "body-lg" | "body";
interface Ctx {
  size: Size;
  tone: "default" | "secondary";
  t: Translate;
  locale: Locale;
  cell?: boolean;
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
const WHOLE_CODE = 16;
const WIDE = /[\u2e80-\u9fff\uac00-\ud7af\uf900-\ufaff\ufe30-\ufe4f\uff00-\uffef]/u;

function width(s: string): number {
  let n = 0;
  for (const ch of s) n += WIDE.test(ch) ? 2 : 1;
  return n;
}

function shortColumns(rows: Inline[][][], columns: number): boolean[] {
  return Array.from({ length: columns }, (_, j) => rows.every((row) => width(plainText(row[j] ?? [])) <= SHORT_COLUMN));
}

const SOFT_BREAK = /(?<=\w[./_:,]+)(?=\w)/;

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
        return <Fragment key={i}>{x.v}</Fragment>;
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
        return ctx.cell && x.v.length <= WHOLE_CODE ? (
          <Code key={i} shape="rounded" className="whitespace-nowrap">
            {x.v}
          </Code>
        ) : (
          <Code key={i} shape="rounded" className={ctx.cell ? undefined : "wrap-anywhere"}>
            {codeText(x.v)}
          </Code>
        );
      case "br":
        return <br key={i} />;
      case "link": {
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

function Item({ item, ordered, n, ctx }: { item: ListItem; ordered: boolean; n: number; ctx: Ctx }) {
  const first = item.c[0];
  const name = first && "c" in first && first.t === "paragraph" ? plainText(first.c) : ctx.t("page.task");
  return (
    <li className="flex gap-12">
      {item.task ? (
        <span className="flex shrink-0 items-center">
          <Checkbox.Box checked={!!item.checked} readOnly aria-label={name} />
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
          {inline(b.c, ctx)}
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
    case "code":
      return <CodeBlock key={i} code={b.code} lang={b.lang} collapseAfter={0} {...(b.title ? { title: b.title } : {})} />;
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
      return (
        <ScrollArea key={i} orientation="horizontal" aria-label={ctx.t("page.table").replace("{columns}", columns)}>
          <Table>
            <Table.Head>
              <Table.Row>
                {b.head.map((c, j) => (
                  <Table.HeaderCell key={j} align={b.align[j] ?? "start"}>
                    {inline(c, cell)}
                  </Table.HeaderCell>
                ))}
              </Table.Row>
            </Table.Head>
            <Table.Body>
              {b.rows.map((row, r) => (
                <Table.Row key={r}>
                  {row.map((c, j) => (
                    <Table.Cell key={j} align={b.align[j] ?? "start"} className={`align-top ${short[j] ? "whitespace-nowrap" : "min-w-140"}`}>
                      {inline(c, cell)}
                    </Table.Cell>
                  ))}
                </Table.Row>
              ))}
            </Table.Body>
          </Table>
        </ScrollArea>
      );
    }
    case "rule":
      return <Separator key={i} className="my-8" />;
  }
}

function blocks(bs: Block[], ctx: Ctx): ReactNode[] {
  return bs.map((b, i) => block(b, i, ctx));
}

export function Markdown({ blocks: bs, className, lang }: { blocks: Block[]; className?: string; lang?: string | undefined }) {
  const t = useText();
  const locale = useSiteLocale();
  return (
    <div lang={lang} className={`flex min-w-0 flex-col gap-16${className ? ` ${className}` : ""}`}>
      {blocks(bs, { size: "body-lg", tone: "default", t, locale })}
    </div>
  );
}
