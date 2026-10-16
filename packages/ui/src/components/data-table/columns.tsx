import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import { formatDate, formatDuration, formatRelative, formatTokens, formatUSD, formatNumber } from "../../format/index.js";
import { StatusDot, type StatusDotProps } from "../status-dot/status-dot.js";
import { Avatar } from "../avatar/avatar.js";
import { Tooltip } from "../tooltip/tooltip.js";
import type { StatusTone } from "../../utils/tone.js";
import { useUiText } from "../../i18n/context.js";

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

function Tokens({ value }: { value: number }) {
  const t = useUiText();
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

function RelativeTime({ value, now }: { value: string | Date; now: Date | number }) {
  const exact = formatDate(value, { time: true, seconds: true });
  return (
    <Tooltip content={`${exact} · ${formatDate(value, { time: true, utc: true })}`} shape="multiline">
      <span className="text-fg-secondary">
        <time dateTime={new Date(value).toISOString()}>{formatRelative(value, now)}</time>
        <span className="sr-only"> {exact}</span>
      </span>
    </Tooltip>
  );
}

export const col = {
  text: <T,>(b: Base<T>, get: Accessor<T, string>) => make(b, get, (v) => v, { text: (r: T) => get(r) }),
  mono: <T,>(b: Base<T>, get: Accessor<T, string>) => make(b, get, (v) => v, { mono: true, text: (r: T) => get(r) }),
  number: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => formatNumber(v), { align: "end" }),
  money: <T,>(b: Base<T>, get: Accessor<T, string>, options: { decimals?: number | undefined } = {}) =>
    make(
      b,
      get,
      (v) => formatUSD(v, options.decimals === undefined ? {} : { decimals: options.decimals }),
      { align: "end", mono: false },
      (a, c, id) => Number(a.getValue(id)) - Number(c.getValue(id)),
    ),
  tokens: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => <Tokens value={v} />, { align: "end" }),
  duration: <T,>(b: Base<T>, get: Accessor<T, number>) => make(b, get, (v) => formatDuration(v), { align: "end" }),
  date: <T,>(b: Base<T>, get: Accessor<T, string | Date>) =>
    make(b, get, (v) => <time dateTime={new Date(v).toISOString()}>{formatDate(v, { time: true })}</time>, {}, "datetime"),
  relative: <T,>(b: Base<T>, get: Accessor<T, string | Date>, options: { now?: Date | number | undefined } = {}) =>
    make(
      b,
      get,
      (v) => <RelativeTime value={v} now={options.now ?? Date.now()} />,
      {},
      (a, c, id) => new Date(a.getValue(id) as string).getTime() - new Date(c.getValue(id) as string).getTime(),
    ),
  status: <T,>(b: Base<T>, get: Accessor<T, { tone: NonNullable<StatusDotProps["tone"]>; label: string }>) =>
    make(b, get, (v) => <StatusDot tone={v.tone}>{v.label}</StatusDot>, { text: undefined }),
  avatar: <T,>(b: Base<T>, get: Accessor<T, { name: string; src?: string | undefined }>) =>
    make(b, get, (v) => (
      <span className="inline-flex min-w-0 items-center gap-8">
        <Avatar name={v.name} {...(v.src ? { src: v.src } : {})} size={24} decorative />
        <span className="truncate">{v.name}</span>
      </span>
    )),
  custom: <T, V>(
    b: Base<T>,
    get: Accessor<T, V>,
    cell: (v: V, row: T) => ReactNode,
    meta: { align?: "start" | "end" | undefined; mono?: boolean | undefined } = {},
  ) => make(b, get, cell, meta),
};
