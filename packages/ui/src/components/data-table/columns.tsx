import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import type { Formatters } from "../../format/index.js";
import { Status, type StatusProps } from "../status/status.js";
import { Avatar } from "../avatar/avatar.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { TruncatedText } from "../text/truncated.js";
import type { StatusTone } from "../../utils/tone.js";
import { useFormatters, useUiText } from "../../i18n/context.js";
import { useHydrating } from "../../hooks/use-hydrating.js";
import { useUtcWhileHydrating } from "../../hooks/use-utc-while-hydrating.js";
import { TIME_COLUMNS } from "./time-columns.js";

function time<C extends object>(column: C): C {
  TIME_COLUMNS.add(column);
  return column;
}

type Base<T = never> = {
  id: string;
  header: ReactNode;
  size?: number | undefined;
  priority?: number | undefined;
  grow?: boolean | undefined;
  minWidth?: number | undefined;
  tone?: ((row: T) => StatusTone | undefined) | undefined;
};
type Accessor<T, V> = (row: T) => V;

function make<T, V>(
  b: Base<T>,
  get: Accessor<T, V>,
  cell: (v: V, row: T) => ReactNode,
  meta: object = {},
  sortingFn?: ColumnDef<T, V>["sortingFn"],
): ColumnDef<T, V> {
  return {
    id: b.id,
    header: () => b.header,
    accessorFn: get,
    cell: (ctx) => cell(ctx.getValue() as V, ctx.row.original),
    ...(b.size ? { size: b.size } : {}),
    ...(sortingFn ? { sortingFn } : {}),
    meta: {
      ...(b.priority !== undefined ? { priority: b.priority } : {}),
      ...(b.grow ? { grow: true } : {}),
      ...(b.minWidth !== undefined ? { minWidth: b.minWidth } : {}),
      ...(b.tone ? { tone: b.tone } : {}),
      ...meta,
    },
  };
}

function Formatted({ format }: { format: (f: Formatters) => string }) {
  return format(useFormatters());
}

function Tokens({ value }: { value: number }) {
  const t = useUiText();
  const { formatNumber, formatTokens } = useFormatters();
  const exact = t("dataTable.tokenCount", "{count} tokens", { count: formatNumber(value) });
  return (
    <Tooltip content={exact}>
      <span>
        <span aria-hidden>{formatTokens(value)}</span>
        <span className="sr-only">{exact}</span>
      </span>
    </Tooltip>
  );
}

function DateTime({ value }: { value: string | Date }) {
  const utc = useUtcWhileHydrating();
  const { formatDate } = useFormatters();
  return <time dateTime={new Date(value).toISOString()}>{formatDate(value, { time: true, utc })}</time>;
}

function RelativeTime({ value, now, live }: { value: string | Date; now: Date | number; live: boolean }) {
  const hydrating = useHydrating();
  const utc = useUtcWhileHydrating();
  const { formatDate, formatRelative } = useFormatters();
  const exact = formatDate(value, { time: true, seconds: true, utc });
  // The server's clock is gone by the time the page hydrates, so a column without a fixed now shows the date itself until then.
  const shown = live && hydrating ? formatDate(value, { time: true, utc }) : formatRelative(value, now, { utc });
  return (
    <Tooltip content={`${exact} · ${formatDate(value, { time: true, utc: true })}`} shape="multiline">
      <span className="text-fg-secondary">
        <time dateTime={new Date(value).toISOString()}>{shown}</time>
        <span className="sr-only"> {exact}</span>
      </span>
    </Tooltip>
  );
}

export const col = {
  text: <T,>(b: Base<T>, get: Accessor<T, string>) => make(b, get, (v) => v, { text: (r: T) => get(r) }),
  mono: <T,>(b: Base<T>, get: Accessor<T, string>) => make(b, get, (v) => v, { mono: true, text: (r: T) => get(r) }),
  number: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => <Formatted format={(f) => f.formatNumber(v)} />, { align: "end" }),
  money: <T,>(b: Base<T>, get: Accessor<T, string>, options: { decimals?: number | undefined } = {}) =>
    make(
      b,
      get,
      (v) => <Formatted format={(f) => f.formatUSD(v, options.decimals === undefined ? {} : { decimals: options.decimals })} />,
      { align: "end", mono: false },
      (a, c, id) => Number(a.getValue(id)) - Number(c.getValue(id)),
    ),
  tokens: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => <Tokens value={v} />, { align: "end" }),
  duration: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => <Formatted format={(f) => f.formatDuration(v)} />, { align: "end" }),
  date: <T,>(b: Base<T>, get: Accessor<T, string | Date>) =>
    time(make(b, get, (v) => <DateTime value={v} />, { text: (r: T, f: Formatters) => f.formatDate(get(r), { time: true }) }, "datetime")),
  relative: <T,>(b: Base<T>, get: Accessor<T, string | Date>, options: { now?: Date | number | undefined } = {}) =>
    time(
      make(
        b,
        get,
        (v) => <RelativeTime value={v} now={options.now ?? Date.now()} live={options.now === undefined} />,
        { text: (r: T, f: Formatters) => f.formatDate(get(r), { time: true }) },
        (a, c, id) => new Date(a.getValue(id) as string).getTime() - new Date(c.getValue(id) as string).getTime(),
      ),
    ),
  status: <T,>(b: Base<T>, get: Accessor<T, { tone: NonNullable<StatusProps["tone"]>; label: string }>) =>
    make(b, get, (v) => <Status tone={v.tone}>{v.label}</Status>, { text: undefined }),
  avatar: <T,>(b: Base<T>, get: Accessor<T, { name: string; src?: string | undefined }>) =>
    make(
      b,
      get,
      (v) => (
        <span className="inline-flex min-w-0 items-center gap-8">
          <Avatar name={v.name} {...(v.src ? { src: v.src } : {})} size={24} decorative />
          <TruncatedText>{v.name}</TruncatedText>
        </span>
      ),
      { text: (r: T) => get(r).name },
    ),
  custom: <T, V>(
    b: Base<T>,
    get: Accessor<T, V>,
    cell: (v: V, row: T) => ReactNode,
    meta: { align?: "start" | "end" | undefined; mono?: boolean | undefined } = {},
  ) => make(b, get, cell, meta),
};
