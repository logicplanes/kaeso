import type * as React from "react";
import { forwardRef, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { mergeRefs } from "../../utils/merge-refs.js";
import { motion, tokenKindSeries, tokenKindTint } from "@logicplanes/kaeso-tokens";
import { Skeleton } from "../skeleton/skeleton.js";
import { Button } from "../button/button.js";
import { ScrollArea } from "../scroll-area/scroll-area.js";
import { SwapText } from "../swap-text/swap-text.js";
import { Table } from "../table/table.js";
import { edgeColor } from "./marks.js";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";

export function wholeTicks(ticks: number[], values: number[]): number[] {
  return values.every(Number.isInteger) ? ticks.filter(Number.isInteger) : ticks;
}
export const VIZ = [
  "var(--k-viz-1)",
  "var(--k-viz-2)",
  "var(--k-viz-3)",
  "var(--k-viz-4)",
  "var(--k-viz-5)",
  "var(--k-viz-6)",
  "var(--k-viz-7)",
  "var(--k-viz-8)",
] as const;
export const VIZ_THIN = [
  "var(--k-viz-thin-1)",
  "var(--k-viz-thin-2)",
  "var(--k-viz-thin-3)",
  "var(--k-viz-thin-4)",
  "var(--k-viz-thin-5)",
  "var(--k-viz-thin-6)",
  "var(--k-viz-thin-7)",
  "var(--k-viz-thin-8)",
] as const;

function hash(s: string) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return Math.abs(h);
}
const NEAR: Record<number, number> = { 1: 6, 6: 1, 2: 5, 5: 2, 3: 7, 7: 3 };

export function seriesIndex(id: string): number {
  return hash(id) % VIZ.length;
}

export function assignSeries(ids: string[]): Map<string, number> {
  const out = new Map<string, number>();
  if (new Set(ids).size === 1) return new Map([[ids[0]!, 0]]);
  const used = new Set<number>();
  const clash = (k: number) => used.has(k) || (NEAR[k] !== undefined && used.has(NEAR[k]!));
  for (const id of ids) {
    if (out.has(id)) continue;
    let i = seriesIndex(id);
    if (clash(i)) {
      const keys = [...VIZ.keys()];
      i = keys.find((k) => !clash(k)) ?? keys.find((k) => !used.has(k)) ?? i;
    }
    used.add(i);
    out.set(id, i);
  }
  return out;
}

export function seriesColors(ids: string[], thin = false): Map<string, string> {
  return new Map([...assignSeries(ids)].map(([id, i]) => [id, (thin ? VIZ_THIN : VIZ)[i]!]));
}

export const seriesColor = (id: string, thin = false) => (thin ? VIZ_THIN : VIZ)[seriesIndex(id)]!;

export type TokenKind = keyof typeof tokenKindSeries;
export const tokenKindColor = (k: TokenKind, thin = false) => {
  if (tokenKindTint[k]) return "var(--k-viz-1-soft)";
  return (thin ? VIZ_THIN : VIZ)[tokenKindSeries[k] - 1]!;
};
export const TOKEN_KIND_ORDER: TokenKind[] = ["input", "cacheRead", "cacheWrite", "output", "reasoning"];
export function tokenKindLabel(kind: TokenKind, t: UiText = englishText): string {
  switch (kind) {
    case "input":
      return t("tokenKind.input", "Input");
    case "output":
      return t("tokenKind.output", "Output");
    case "cacheRead":
      return t("tokenKind.cacheRead", "Cache read");
    case "cacheWrite":
      return t("tokenKind.cacheWrite", "Cache write");
    case "reasoning":
      return t("tokenKind.reasoning", "Reasoning");
  }
}

export function useSize<T extends HTMLElement>(): [(el: T | null) => void, { width: number; height: number }] {
  const [el, setEl] = useState<T | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    if (!el) return;
    const set = () => setSize({ width: Math.round(el.clientWidth), height: Math.round(el.clientHeight) });
    set();
    return observeResize([el], set);
  }, [el]);
  return [setEl, size];
}

const ease = (t: number) => 1 - Math.pow(1 - t, 3);
const reducedMotion = () => typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
const mix = (from: number[], to: number[], t: number) => (t >= 1 ? to : to.map((v, i) => (from[i] ?? v) + (v - (from[i] ?? v)) * ease(t)));

export function tweens(from: number[], to: number[]): boolean {
  return from.length === to.length && from.some((v) => v !== 0) && to.some((v) => v !== 0) && from.some((v, i) => v !== to[i]) && !reducedMotion();
}

export function useTweened(values: number[], still = false): number[] {
  const key = values.join(",");
  const [tween, setTween] = useState({ key, from: values, to: values, t: 1, still });
  let current = tween;
  if (tween.key !== key || tween.still !== still) {
    const drawn = mix(tween.from, tween.to, tween.t);
    const animate = !still && !tween.still && tweens(drawn, values);
    current = animate ? { key, from: drawn, to: values, t: 0, still } : { key, from: values, to: values, t: 1, still };
    setTween(current);
  }
  const running = current.t < 1;
  useEffect(() => {
    if (!running) return;
    const start = performance.now();
    let raf = 0;
    const step = () => {
      const t = Math.max(0, Math.min(1, (performance.now() - start) / motion.duration.chart));
      setTween((s) => (s.key === key ? { ...s, t } : s));
      if (t < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [key, running]);
  return mix(current.from, current.to, current.t);
}

export function useTweenedScale(values: number[], top: number, still = false): { values: number[]; top: number } {
  const any = values.some((v) => v !== 0);
  const drawn = useTweened(any ? [...values, top] : values, still);
  return any ? { values: drawn.slice(0, -1), top: drawn[drawn.length - 1] ?? top } : { values: drawn, top };
}

export interface ChartState {
  loading?: boolean | undefined;
  error?: ReactNode | undefined;
  onRetry?: (() => void) | undefined;
  empty?: boolean | undefined;
  emptyText?: ReactNode | undefined;
}

export interface TableData {
  columns: string[];
  rows: (string | number)[][];
}

export interface ChartFrameProps {
  label: string;
  height: number;
  children: ReactNode;
  table?: TableData | undefined;
  legend?: ReactNode | undefined;
  state?: ChartState | undefined;
  className?: string | undefined;
  toggle?: boolean | undefined;
}

export const ChartFrame = forwardRef<HTMLElement, ChartFrameProps>(function ChartFrame(
  { label, height, children, table, legend, state = {}, className, toggle = true },
  ref,
) {
  const [asTable, setAsTable] = useState(false);
  const t = useUiText();
  const failed = !!state.error;
  const blank = !!state.empty;
  const showTable = asTable && !!table && !failed && !blank;
  const toTable = t("chartFrame.viewAsTable", "View as table");
  const toChart = t("chartFrame.viewAsChart", "View as chart");
  return (
    <figure
      ref={ref}
      className={cn("@container flex w-full min-w-0 flex-col gap-12", className)}
      aria-label={label}
      {...(state.loading ? { "aria-busy": true } : {})}
    >
      {failed ? (
        <div className="flex flex-col items-center justify-center gap-12 px-24 text-center" style={{ height }}>
          <p role="alert" className="text-body text-pretty text-fg-secondary">
            {state.onRetry
              ? t("chartFrame.loadFailed", "Couldn't load this chart")
              : t("chartFrame.loadFailedReload", "Couldn't load this chart. Reload the page to try again.")}
          </p>
          {state.onRetry && (
            <Button size="sm" variant="secondary" onClick={state.onRetry}>
              {t("chartFrame.tryAgain", "Try again")}
            </Button>
          )}
        </div>
      ) : showTable ? (
        <div style={{ height }}>
          <ScrollArea className="h-full" aria-label={t("chartFrame.tableLabel", "{label}, table", { label })}>
            <DataTablePlain table={table} sticky />
          </ScrollArea>
        </div>
      ) : (
        <div className="relative" style={{ height }}>
          {state.loading && state.empty ? (
            <Skeleton className="absolute inset-0" />
          ) : (
            <div className={cn("absolute inset-0", state.loading && "opacity-40")}>
              <div className={cn("absolute inset-0", state.loading && "animate-pulse-quiet")}>{children}</div>
            </div>
          )}
          {state.empty && !state.loading && (
            <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-24 text-center text-body text-pretty text-fg-secondary">
              {state.emptyText ?? t("chartFrame.noData", "No data for this period")}
            </div>
          )}
        </div>
      )}
      {!failed && (legend || (toggle && table && !blank)) && (
        <div className="flex flex-col gap-8 @chart-legend:flex-row @chart-legend:gap-12">
          {!showTable && legend && <div className="min-w-0 @chart-legend:flex-1">{legend}</div>}
          {toggle && table && !blank && (
            <button
              type="button"
              onClick={() => setAsTable((v) => !v)}
              className="relative ml-auto inline-flex h-28 shrink-0 cursor-pointer items-center self-end rounded-pill px-10 text-small font-medium text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring tap-target"
            >
              <SwapText reserve={[toTable, toChart]}>{showTable ? toChart : toTable}</SwapText>
            </button>
          )}
        </div>
      )}
      {table && !showTable && (
        <div className="sr-only">
          <DataTablePlain table={table} caption={label} />
        </div>
      )}
    </figure>
  );
});
ChartFrame.displayName = "ChartFrame";

const NUMERIC = /^[^\p{L}]*\d[^\p{L}]*\p{L}{0,3}$/u;

function DataTablePlain({ table, caption, sticky }: { table: TableData; caption?: string | undefined; sticky?: boolean | undefined }) {
  const end = table.columns.map(
    (_, j) => j > 0 && table.rows.every((r) => typeof r[j] === "number" || NUMERIC.test(String(r[j] ?? "").trim()) || r[j] === "–"),
  );
  return (
    <Table>
      {caption && <caption className="sr-only">{caption}</caption>}
      <Table.Head>
        <Table.Row>
          {table.columns.map((c, i) => (
            <Table.HeaderCell key={c} align={end[i] ? "end" : "start"} className={cn(sticky && "sticky top-0 z-10 bg-panel")}>
              {c}
            </Table.HeaderCell>
          ))}
        </Table.Row>
      </Table.Head>
      <Table.Body>
        {table.rows.map((r, i) => (
          <Table.Row key={i}>
            {r.map((v, j) => (
              <Table.Cell key={j} align={end[j] ? "end" : "start"}>
                {v}
              </Table.Cell>
            ))}
          </Table.Row>
        ))}
      </Table.Body>
    </Table>
  );
}

export interface LegendItem {
  id: string;
  label: ReactNode;
  color: string;
  value?: ReactNode | undefined;
}

export interface SwatchProps {
  color: string;
  shape?: "square" | "line" | undefined;
  off?: boolean | undefined;
}

export const Swatch = forwardRef<HTMLSpanElement, SwatchProps>(function Swatch({ color, shape = "square", off }, ref) {
  const edge = edgeColor(color);
  return (
    <span
      ref={ref}
      aria-hidden
      data-off={off ? "" : undefined}
      className={cn("shrink-0 data-mark transition-control", shape === "line" ? "h-2 w-12 rounded-pill" : "size-10 rounded-bar")}
      style={
        off
          ? shape === "line"
            ? { outline: `1px solid ${edge ?? color}`, outlineOffset: 1 }
            : { boxShadow: `inset 0 0 0 1px ${edge ?? color}` }
          : { background: color, ...(edge && shape === "square" ? { boxShadow: `inset 0 0 0 1px ${edge}` } : {}) }
      }
    />
  );
});
Swatch.displayName = "Swatch";

export const tickLabel = (format: (v: number) => string) => (v: number) => format(v).replace(/[.,]00(?=\D*$)/, "");

export function readableTicks(scale: { ticks: (count: number) => number[]; domain: () => number[] }, label: (v: number) => string, count = 4): number[] {
  for (let n = count; n >= 1; n--) {
    const ticks = scale.ticks(n);
    if (new Set(ticks.map(label)).size === ticks.length) return ticks;
  }
  const [lo = 0, hi = 0] = scale.domain();
  return label(lo) === label(hi) ? [lo] : [lo, hi];
}

export type AxisDate = Date | string;

function dayNumber(d: AxisDate): number {
  if (d instanceof Date) return Number.isNaN(d.getTime()) ? NaN : Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 86_400_000;
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(d);
  return m ? Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])) / 86_400_000 : NaN;
}
const yearOfDay = (n: number) => new Date(n * 86_400_000).getUTCFullYear();

export function yearStarts(dates: readonly AxisDate[]): number[] {
  const days = dates.map(dayNumber);
  const out: number[] = [];
  days.forEach((d, i) => {
    const prev = i > 0 ? days[i - 1]! : days.length > 1 ? 2 * d - days[1]! : NaN;
    if (Number.isFinite(d) && Number.isFinite(prev) && yearOfDay(d) !== yearOfDay(prev)) out.push(i);
  });
  return out;
}

const WIDE = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\u3000-\u303f\uff00-\uffef]/u;

export function labelWidth(label: string): number {
  let em = 0;
  for (const ch of label) em += WIDE.test(ch) ? 1 : /\p{Lu}/u.test(ch) ? 0.8 : /\p{Ll}/u.test(ch) ? 0.6 : /\p{Nd}/u.test(ch) ? 0.66 : /\s/.test(ch) ? 0.3 : 0.4;
  return em * AXIS.label.fontSize;
}

const LABEL_GAP = 8;

export function placeTimeLabels(
  ticks: { key: number; at: number; label: string }[],
  right: number,
): { key: number; x: number; anchor: "start" | "middle" | "end"; label: string }[] {
  const placed = ticks.map((t) => {
    const width = labelWidth(t.label);
    const anchor: "start" | "middle" | "end" = t.at - width / 2 < 0 ? "start" : t.at + width / 2 > right ? "end" : "middle";
    const x = anchor === "start" ? Math.max(t.at, 0) : t.at;
    const left = anchor === "start" ? x : anchor === "end" ? x - width : x - width / 2;
    return { key: t.key, x, anchor, label: t.label, left, right: left + width };
  });
  const clear = (a: { right: number } | undefined, b: { left: number } | undefined) => !a || !b || a.right + LABEL_GAP <= b.left;
  if (placed.length > 2 && !clear(placed[0], placed[1])) placed.splice(1, 1);
  if (placed.length > 2 && !clear(placed[placed.length - 2], placed[placed.length - 1])) placed.splice(placed.length - 2, 1);
  const kept: typeof placed = [];
  for (const l of placed) if (clear(kept[kept.length - 1], l)) kept.push(l);
  return kept.map(({ key, x, anchor, label }) => ({ key, x, anchor, label }));
}

export function axisLabelIndexes(
  labels: string[],
  step: number,
  edges: { start: number; end: number } = { start: Infinity, end: Infinity },
  dates?: readonly AxisDate[] | undefined,
): number[] {
  const n = labels.length;
  if (n === 0) return [];
  const fits = (i: number) => labelWidth(labels[i]!) / 2 <= Math.min(edges.start + i * step, edges.end + (n - 1 - i) * step) + 1e-6;
  const room = Math.max(48, Math.max(...labels.map(labelWidth)) + 16);
  const every = step > 0 ? Math.max(1, Math.ceil(room / step - 1e-6)) : n;
  const starts = dates && dates.length === n ? yearStarts(dates) : [];
  let stride = every;
  for (let s = every; every > 1 && s <= every * 2; s++) {
    if (starts.every((i) => (i - starts[0]!) % s === 0)) {
      stride = s;
      break;
    }
  }
  const score = (p: number) => [
    starts.filter((i) => i % stride === p && fits(i)).length,
    (n - 1) % stride === p && fits(n - 1) ? 1 : 0,
    p === 0 && fits(0) ? 1 : 0,
    n - 1 - ((n - 1 - p) % stride),
  ];
  let best = 0;
  for (let p = 1; p < Math.min(stride, n); p++) {
    const a = score(p);
    const b = score(best);
    const k = a.findIndex((v, j) => v !== b[j]);
    if (k >= 0 && a[k]! > b[k]!) best = p;
  }
  const out: number[] = [];
  for (let i = best; i < n; i += stride) if (fits(i)) out.push(i);
  return out;
}

export interface LegendProps {
  items: LegendItem[];
  hidden?: Set<string> | undefined;
  onToggle?: ((id: string) => void) | undefined;
  onHover?: ((id: string | null) => void) | undefined;
  shape?: "square" | "line" | undefined;
}

export const Legend = forwardRef<HTMLUListElement, LegendProps>(function Legend({ items, hidden, onToggle, onHover, shape = "square" }, ref) {
  const t = useUiText();
  return (
    <ul ref={ref} className={cn("flex flex-wrap items-center", onToggle ? "gap-4" : "gap-x-16 gap-y-4")} aria-label={t("chartLegend.label", "Legend")}>
      {items.map((it) => {
        const off = hidden?.has(it.id);
        const Comp = onToggle ? "button" : "span";
        return (
          <li key={it.id}>
            <Comp
              {...(onToggle ? { type: "button" as const, "aria-pressed": !off, onClick: () => onToggle(it.id) } : {})}
              onMouseEnter={() => onHover?.(it.id)}
              onMouseLeave={() => onHover?.(null)}
              onFocus={() => onHover?.(it.id)}
              onBlur={() => onHover?.(null)}
              className={cn(
                "inline-flex h-28 items-center gap-8 text-small font-medium transition-control",
                onToggle && "relative cursor-pointer rounded-pill px-10 hover:bg-fill-1 focus-ring tap-target",
                off ? "text-fg-tertiary" : "text-fg",
              )}
            >
              <Swatch color={it.color} shape={shape} off={off} />
              {it.label}
              {it.value !== undefined && <span className="text-fg-secondary tabular">{it.value}</span>}
            </Comp>
          </li>
        );
      })}
    </ul>
  );
});
Legend.displayName = "Legend";

export interface TooltipRow {
  id: string;
  label: ReactNode;
  color: string;
  value: ReactNode;
  share?: ReactNode | undefined;
}

export interface ChartTooltipProps {
  x: number;
  xLeft?: number | undefined;
  y: number;
  width: number;
  heading: ReactNode;
  rows: TooltipRow[];
  visible: boolean;
}

export const ChartTooltip = forwardRef<HTMLDivElement, ChartTooltipProps>(function ChartTooltip({ x, xLeft = x, y, width, heading, rows, visible }, ref) {
  const el = useRef<HTMLDivElement | null>(null);
  const root = useMemo(() => mergeRefs(el, ref), [ref]);
  const [own, setOwn] = useState({ w: 0, h: 0, room: Infinity });
  useLayoutEffect(() => {
    const node = el.current;
    const next = { w: node?.offsetWidth ?? 0, h: node?.offsetHeight ?? 0, room: (node?.offsetParent as HTMLElement | null)?.clientHeight ?? Infinity };
    if (next.w !== own.w || next.h !== own.h || next.room !== own.room) setOwn(next);
  }, [own, heading, rows, visible]);
  const flip = x > width / 2;
  const wanted = flip ? xLeft - 16 - own.w : x + 16;
  const left = own.w ? Math.round(Math.max(0, Math.min(wanted, width - own.w))) : undefined;
  const top = Math.round(Math.max(0, Math.min(y - 24, own.room - own.h)));
  return (
    <div
      ref={root}
      role="presentation"
      aria-hidden
      className={cn(
        "pointer-events-none absolute z-10 w-max min-w-160 max-w-360 rounded-tooltip bg-raised p-10 text-small shadow-raised opacity-motion @chart-legend:min-w-200",
        visible && own.w ? "opacity-100" : "opacity-0",
      )}
      style={{
        ...(left !== undefined ? { left } : flip ? { right: Math.round(width - xLeft + 16) } : { left: Math.round(x + 16) }),
        top,
      }}
    >
      <div className="mb-6 text-caption font-medium text-fg tabular">{heading}</div>
      <div className="flex flex-col gap-4">
        {rows.map((r) => (
          <div key={r.id} className="flex items-center gap-8">
            <Swatch color={r.color} />
            <span className="min-w-0 flex-1 truncate pr-12 text-fg-secondary">{r.label}</span>
            <span className="font-medium text-fg tabular">{r.value}</span>
            {r.share !== undefined && <span className="w-40 text-right text-fg-tertiary tabular">{r.share}</span>}
          </div>
        ))}
      </div>
    </div>
  );
});
ChartTooltip.displayName = "ChartTooltip";

export const AXIS = {
  grid: { stroke: "var(--k-line)", strokeDasharray: "3 3", strokeWidth: 1 },
  baseline: { stroke: "var(--k-line-strong)", strokeWidth: 1 },
  label: { fill: "var(--k-text-tertiary)", fontSize: 12, fontWeight: 500, style: { fontVariantNumeric: "tabular-nums" as const } },
};

export const KAESO_CHARTS_VARIANTS = {
  chrome: { default: { classes: "", description: "Dashed 3 3 grid, tertiary 12px labels, a strong baseline; no frame" } },
} as const;
export const KAESO_CHARTS_DEFAULT_VARIANTS = { chrome: "default" } as const;

export function chartKeyIndex(
  key: string,
  index: number | null,
  count: number,
  { vertical = false, start = count - 1 }: { vertical?: boolean | undefined; start?: number | undefined } = {},
): number | null | undefined {
  if (count <= 0) return undefined;
  const back = vertical ? "ArrowUp" : "ArrowLeft";
  const forward = vertical ? "ArrowDown" : "ArrowRight";
  if (key === back) return index === null ? start : Math.max(0, index - 1);
  if (key === forward) return index === null ? start : Math.min(count - 1, index + 1);
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  if (key === "Escape") return index === null ? undefined : null;
  return undefined;
}

export function useChartKeys(count: number, options: { vertical?: boolean | undefined; start?: number | undefined } = {}) {
  const [index, setIndex] = useState<number | null>(null);
  const start = options.start ?? count - 1;
  const props = {
    tabIndex: count > 0 ? 0 : -1,
    onFocus: (e: React.FocusEvent<HTMLElement>) => {
      if (e.target !== e.currentTarget) return;
      let keyboard = true;
      try {
        keyboard = e.currentTarget.matches(":focus-visible");
      } catch {}
      if (keyboard && !document.documentElement.hasAttribute("data-k-pointer")) setIndex(Math.max(0, start));
    },
    onBlur: () => setIndex(null),
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => {
      const next = chartKeyIndex(e.key, index, count, { vertical: !!options.vertical, start });
      if (next === undefined) return;
      e.preventDefault();
      if (next === null) e.stopPropagation();
      setIndex(next);
    },
  };
  return { index: index !== null && index < count ? index : null, props };
}

export interface ChartLiveProps {
  children?: ReactNode | undefined;
}

export const ChartLive = forwardRef<HTMLDivElement, ChartLiveProps>(function ChartLive({ children }, ref) {
  return (
    <div ref={ref} className="sr-only" aria-live="polite" aria-atomic>
      {children}
    </div>
  );
});
ChartLive.displayName = "ChartLive";

export const liveLine = (heading: ReactNode, rows: { label: ReactNode; value: ReactNode }[], t: UiText = englishText) =>
  t("chart.liveLine", "{heading}: {values}", {
    heading: String(heading),
    values: rows.map((r) => t("chart.liveValue", "{label} {value}", { label: String(r.label), value: String(r.value) })).join(t("chart.listSeparator", ", ")),
  });
