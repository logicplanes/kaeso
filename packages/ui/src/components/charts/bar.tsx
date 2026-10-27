import { forwardRef, useState } from "react";
import { scaleBand } from "d3-scale";
import { radius } from "@logicplanes/kaeso-tokens";
import { scaleLinear } from "./linear-scale.js";
import { cn } from "../../utils/cn.js";
import { formatNumber, formatPercent } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import {
  AXIS,
  axisLabelIndexes,
  type AxisDate,
  ChartFrame,
  ChartLive,
  ChartTooltip,
  labelWidth,
  Legend,
  liveLine,
  seriesColors,
  readableTicks,
  tickLabel,
  useChartKeys,
  useSize,
  useTweened,
  wholeTicks,
  type ChartState,
} from "./core.js";
import { axisRoom, compactAxis, finite, onlyZero, fitLabel, hasPoints, valueExtent } from "./domain.js";
import { edgeColor } from "./marks.js";

export const KAESO_BAR_CHART_VARIANTS = {
  layout: {
    grouped: { classes: "", description: "Bars side by side per category (default)" },
    stacked: { classes: "", description: "Bars stacked per category" },
    horizontal: { classes: "", description: "Horizontal bars for rankings" },
  },
} as const;
export const KAESO_BAR_CHART_DEFAULT_VARIANTS = { layout: "grouped" } as const;

export interface BarSeries {
  id: string;
  label: string;
  values: number[];
  color?: string | undefined;
}

export interface BarChartProps extends ChartState {
  categories: string[];
  series: BarSeries[];
  layout?: keyof typeof KAESO_BAR_CHART_VARIANTS.layout | undefined;
  height?: number | undefined;
  formatValue?: ((v: number) => string) | undefined;
  highlight?: number | undefined;
  axisLabels?: string[] | undefined;
  dates?: readonly AxisDate[] | undefined;
  shares?: boolean | undefined;
  partial?: { first?: boolean | undefined; last?: boolean | undefined; ongoing?: boolean | undefined } | undefined;
  "aria-label": string;
  className?: string | undefined;
}

const GAP = 2;
const PAD = 8;
const CATEGORY_FONT = 13;

type Span = { from: number; to: number };

function stackSpans(values: number[], at: (n: number) => number, gap: number): (Span | null)[] {
  const spans: (Span | null)[] = values.map(() => null);
  for (const sign of [1, -1]) {
    let acc = 0;
    const idx = values.map((v, i) => (Number.isFinite(v) && v * sign > 0 ? i : -1)).filter((i) => i >= 0);
    const raw = idx.map((i) => {
      const a = at(acc);
      acc += values[i]!;
      return { i, a, b: at(acc) };
    });
    const len = raw.map((r, k) => Math.abs(r.b - r.a) - (k > 0 ? gap : 0));
    let debt = 0;
    len.forEach((l, k) => {
      if (l < 2) {
        debt += 2 - l;
        len[k] = 2;
      }
    });
    if (debt > 0) {
      const big = len.indexOf(Math.max(...len));
      if (big >= 0) len[big] = Math.max(2, len[big]! - debt);
    }
    const dir = raw.length ? Math.sign(raw[raw.length - 1]!.b - raw[0]!.a) || 1 : 1;
    let edge = raw[0]?.a ?? 0;
    raw.forEach((r, k) => {
      const start = k > 0 ? edge + dir * gap : edge;
      const end = start + dir * len[k]!;
      spans[r.i] = { from: start, to: end };
      edge = end;
    });
  }
  return spans;
}

function barPath(x: number, y: number, w: number, h: number, r: number, round: "top" | "bottom" | "left" | "right" | "none") {
  if (w <= 0 || h <= 0) return "";
  const rr = Math.min(r, round === "left" || round === "right" ? h / 2 : w / 2, round === "left" || round === "right" ? w : h);
  if (rr <= 0 || round === "none") return `M${x} ${y}h${w}v${h}h${-w}z`;
  const X = x + w;
  const Y = y + h;
  switch (round) {
    case "top":
      return `M${x} ${Y}V${y + rr}Q${x} ${y} ${x + rr} ${y}H${X - rr}Q${X} ${y} ${X} ${y + rr}V${Y}z`;
    case "bottom":
      return `M${x} ${y}V${Y - rr}Q${x} ${Y} ${x + rr} ${Y}H${X - rr}Q${X} ${Y} ${X} ${Y - rr}V${y}z`;
    case "right":
      return `M${x} ${y}H${X - rr}Q${X} ${y} ${X} ${y + rr}V${Y - rr}Q${X} ${Y} ${X - rr} ${Y}H${x}z`;
    case "left":
      return `M${X} ${y}H${x + rr}Q${x} ${y} ${x} ${y + rr}V${Y - rr}Q${x} ${Y} ${x + rr} ${Y}H${X}z`;
  }
}

export const BarChart = forwardRef<HTMLDivElement, BarChartProps>(function BarChart(
  {
    categories,
    series,
    layout = "grouped",
    height = 240,
    formatValue,
    highlight,
    axisLabels,
    dates,
    shares = true,
    partial,
    className,
    "aria-label": ariaLabel,
    ...state
  },
  ref,
) {
  const t = useUiText();
  const format = formatValue ?? ((v: number) => formatNumber(v));
  const write = (v: number) => (Number.isFinite(v) ? format(v) : "–");
  const [box, size] = useSize<HTMLDivElement>();
  const [hover, setHover] = useState<{ i: number; y: number } | null>(null);
  const [focus, setFocus] = useState<string | null>(null);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const horizontal = layout === "horizontal";
  const stacked = layout === "stacked";
  const shown = series.filter((s) => !hidden.has(s.id));
  const flat = shown.flatMap((s) => categories.map((_, ci) => s.values[ci] ?? NaN));
  const allHidden = series.length > 0 && shown.length === 0;
  const empty = state.empty ?? (allHidden || categories.length === 0 || !hasPoints(flat));
  const sums = (sign: number) =>
    categories.map((_, ci) => shown.reduce((a, s) => a + (Number.isFinite(s.values[ci]) && (s.values[ci] ?? 0) * sign > 0 ? s.values[ci]! : 0), 0));
  const extent = valueExtent(stacked ? [...sums(1), ...sums(-1)] : flat);
  const axis = scaleLinear().domain(extent).nice(4);
  const [niceLo = 0, niceHi = 1] = axis.domain();
  const drawn = useTweened([...flat.map((v) => (Number.isFinite(v) ? v : 0)), niceLo, niceHi], !!state.loading);
  const lo = drawn[drawn.length - 2] ?? niceLo;
  const hi = drawn[drawn.length - 1] ?? niceHi;
  const vals = shown.map((_, si) => categories.map((_, ci) => drawn[si * categories.length + ci] ?? 0));
  const palette = seriesColors(series.map((s) => s.id));
  const color = (s: BarSeries) => s.color ?? palette.get(s.id)!;
  const axisLabel = tickLabel(formatValue ?? compactAxis);
  const ticks = empty ? [] : onlyZero(flat) ? [0] : wholeTicks(readableTicks(axis, axisLabel), finite(flat)).filter((v) => v >= lo - 1e-9 && v <= hi + 1e-9);
  const total = (ci: number) => {
    const known = finite(shown.map((s) => s.values[ci] ?? NaN));
    return known.length ? known.reduce((a, n) => a + n, 0) : NaN;
  };
  const valueLabel = (ci: number, si = 0) => write(stacked ? total(ci) : (shown[si]?.values[ci] ?? NaN));
  const widestCategory = Math.max(0, ...categories.map((c) => labelWidth(c))) * (CATEGORY_FONT / AXIS.label.fontSize);
  const cap = size.width ? size.width * 0.4 : Infinity;
  const M = horizontal
    ? {
        top: 0,
        right: axisRoom(
          categories.flatMap((_, ci) => (stacked ? [valueLabel(ci)] : shown.map((_, si) => valueLabel(ci, si)))),
          { gap: 10, min: 40, max: cap },
        ),
        bottom: 0,
        left: Math.round(Math.min(160, cap, Math.max(48, widestCategory + 12 + 4))),
      }
    : { top: 8, right: axisRoom(ticks.map(axisLabel), { max: cap }), bottom: 28, left: 0 };
  const w = Math.max(0, size.width - M.left - M.right);
  const h = Math.max(0, height - M.top - M.bottom);
  const band = scaleBand<string>()
    .domain(categories)
    .range(horizontal ? [0, h] : [0, w])
    .paddingInner(0.24)
    .paddingOuter(0.12);
  axis.range(horizontal ? [0, w] : [h, 0]);
  const scale = axis.copy().domain([lo, hi === lo ? lo + 1 : hi]);
  const v = (n: number) => {
    const p = scale(n);
    return horizontal ? Math.max(0, Math.min(w, p)) : Math.max(0, Math.min(h, p));
  };
  const zero = Math.round(v(0));
  const grouped = !stacked && shown.length > 1;
  const inner = scaleBand<string>()
    .domain(shown.map((s) => s.id))
    .range([0, band.bandwidth()])
    .paddingInner(0.12);
  const MAX = horizontal ? 20 : stacked || shown.length <= 1 ? 40 : 20;
  const barW = Math.max(4, Math.min(MAX, Math.round(grouped ? inner.bandwidth() : band.bandwidth())));
  const groupW = grouped ? shown.length * barW + (shown.length - 1) * 4 : barW;
  const centers = [categories[0], categories[categories.length - 1]].map((c) => (c === undefined ? 0 : (band(c) ?? 0) + band.bandwidth() / 2));
  const labelled = new Set(
    horizontal
      ? []
      : axisLabelIndexes(
          categories.map((c, ci) => axisLabels?.[ci] ?? c),
          band.step(),
          { start: M.left + centers[0]!, end: w - centers[1]! + M.right },
          dates,
        ),
  );
  const lastCategory = categories.length - 1;
  const isPartial = (ci: number) => (!!(partial?.last || partial?.ongoing) && ci === lastCategory) || (!!partial?.first && ci === 0);
  const heading = (ci: number) => {
    const name = categories[ci] ?? "";
    if (partial?.ongoing && ci === lastCategory) return t("chart.soFar", "{heading}, so far", { heading: name });
    return isPartial(ci) ? t("chart.partial", "{heading}, partial", { heading: name }) : name;
  };
  const table = {
    columns: [t("barChart.category", "Category"), ...series.map((s) => s.label)],
    rows: categories.map((_, ci) => [heading(ci), ...series.map((s) => write(s.values[ci] ?? NaN))]),
  };
  const quiet = !!state.loading || empty || !!state.error;
  const keys = useChartKeys(quiet ? 0 : categories.length, horizontal ? { vertical: true, start: 0 } : {});
  const active = quiet ? null : (hover?.i ?? keys.index ?? (highlight !== undefined && highlight < categories.length ? highlight : null));
  const totals = categories.map((_, ci) => finite(shown.map((s) => s.values[ci] ?? NaN)).reduce((a, n) => a + Math.abs(n), 0));
  const spans = categories.map((_, ci) =>
    stacked
      ? stackSpans(
          vals.map((sv) => sv[ci] ?? 0),
          v,
          GAP,
        )
      : null,
  );
  const barEnd = (ci: number) => {
    if (stacked) return Math.max(zero, ...(spans[ci] ?? []).map((s) => (s ? Math.max(s.from, s.to) : zero)));
    return Math.max(zero, ...vals.map((sv) => v(sv[ci] ?? 0)));
  };
  const barTop = (ci: number) => {
    if (stacked) return Math.min(zero, ...(spans[ci] ?? []).map((s) => (s ? Math.min(s.from, s.to) : zero)));
    return Math.min(zero, ...vals.map((sv) => v(sv[ci] ?? 0)));
  };
  const anchor = (ci: number) => {
    const b = Math.round(band(categories[ci]!) ?? 0);
    if (horizontal) {
      const rowTop = M.top + Math.round(b + band.bandwidth() / 2 - groupW / 2 - PAD);
      const widestValue = Math.max(0, ...(stacked ? [valueLabel(ci)] : shown.map((_, si) => valueLabel(ci, si))).map(labelWidth));
      return { x: M.left + Math.round(barEnd(ci) + 10 + widestValue), y: rowTop + 24 };
    }
    const lit = Math.min(band.bandwidth(), groupW + PAD * 2);
    const center = M.left + b + band.bandwidth() / 2;
    return {
      x: Math.round(center + lit / 2) - 8,
      xLeft: Math.round(center - lit / 2) + 8,
      y: hover && hover.i === ci ? hover.y : M.top + Math.round(barTop(ci)),
    };
  };
  const rowsFor = (ci: number) =>
    shown.map((s) => {
      const value = s.values[ci] ?? NaN;
      return {
        id: s.id,
        label: s.label,
        color: color(s),
        value: write(value),
        ...(shares && shown.length > 1 && totals[ci] && Number.isFinite(value)
          ? { share: formatPercent(Math.round((Math.abs(value) / (totals[ci] ?? 1)) * 100) / 100, 0) }
          : {}),
      };
    });
  const toggle = (id: string) => {
    const next = new Set(hidden);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setHidden(next);
  };
  const mark = (s: BarSeries, ci: number, d: string, key: string) => {
    const fill = color(s);
    const edge = edgeColor(fill);
    const part = isPartial(ci);
    return (
      <path
        key={key}
        d={d}
        fill={fill}
        className="opacity-motion"
        opacity={focus && focus !== s.id ? 0.3 : 1}
        {...(part
          ? { fillOpacity: 0.32, stroke: edge ?? fill, strokeWidth: 1.5, strokeDasharray: "4 4", "data-partial": "" }
          : edge
            ? { stroke: edge, strokeWidth: 1 }
            : {})}
      />
    );
  };
  return (
    <div ref={ref} className={cn("min-w-0", className)}>
      <ChartFrame
        label={ariaLabel}
        height={height}
        table={table}
        state={{
          ...state,
          empty,
          ...(allHidden && !state.empty ? { emptyText: t("chart.allHidden", "Every series is hidden. Choose one in the legend to show it.") } : {}),
        }}
        legend={
          series.length > 1 ? (
            <Legend items={series.map((s) => ({ id: s.id, label: s.label, color: color(s) }))} hidden={hidden} onToggle={toggle} onHover={setFocus} />
          ) : undefined
        }
      >
        <div ref={box} role="group" aria-label={ariaLabel} {...keys.props} className="absolute inset-0 rounded-cell focus-ring">
          {size.width > 0 && (
            <svg width={size.width} height={height} className="block overflow-visible" role="presentation">
              <g transform={`translate(${M.left},${M.top})`}>
                {!horizontal &&
                  ticks.map((tick) => (
                    <g key={tick} transform={`translate(0,${Math.round(v(tick)) + 0.5})`}>
                      {tick !== 0 && <line x2={w} {...AXIS.grid} />}
                      <text x={w + 12} dy="0.32em" {...AXIS.label}>
                        {axisLabel(tick)}
                      </text>
                    </g>
                  ))}
                {!empty &&
                  categories.map((c, ci) => {
                    const b = Math.round(band(c) ?? 0);
                    const gap = Math.round((band.step() - band.bandwidth()) / 2);
                    const lit = Math.min(band.bandwidth(), groupW + PAD * 2);
                    const center = b + band.bandwidth() / 2;
                    const start = Math.round(center - groupW / 2);
                    const outer = (sign: number) => {
                      const ids = spans[ci]?.map((sp, si) => (sp && Math.sign(vals[si]?.[ci] ?? 0) === sign ? si : -1)).filter((si) => si >= 0) ?? [];
                      return ids[ids.length - 1];
                    };
                    return (
                      <g key={c}>
                        <rect
                          aria-hidden
                          className="opacity-motion"
                          opacity={active === ci ? 1 : 0}
                          fill="var(--k-fill-1)"
                          rx={parseFloat(radius.item)}
                          {...(horizontal
                            ? { x: -M.left, y: Math.round(center - groupW / 2 - PAD), width: size.width, height: groupW + PAD * 2 }
                            : { x: Math.round(center - lit / 2), y: 0, width: lit, height: h })}
                        />
                        {shown.map((s, si) => {
                          const val = vals[si]?.[ci] ?? 0;
                          if (!Number.isFinite(s.values[ci] ?? NaN) || val === 0) return null;
                          if (stacked) {
                            const sp = spans[ci]?.[si];
                            if (!sp) return null;
                            const a = Math.min(sp.from, sp.to);
                            const len = Math.abs(sp.to - sp.from);
                            const isOuter = si === outer(Math.sign(val));
                            if (horizontal) {
                              return mark(
                                s,
                                ci,
                                barPath(a, Math.round(center - barW / 2), len, barW, 4, isOuter ? (val > 0 ? "right" : "left") : "none"),
                                s.id,
                              );
                            }
                            return mark(s, ci, barPath(start, a, barW, len, 4, isOuter ? (val > 0 ? "top" : "bottom") : "none"), s.id);
                          }
                          const end = v(val);
                          const len = Math.max(2, Math.abs(end - zero));
                          if (horizontal) {
                            const y = Math.round(center - groupW / 2 + si * (barW + 4));
                            const x = val > 0 ? zero : zero - len;
                            return mark(s, ci, barPath(x, y, len, barW, 4, val > 0 ? "right" : "left"), s.id);
                          }
                          const x = start + si * (barW + 4);
                          const y = val > 0 ? zero - len : zero;
                          return mark(s, ci, barPath(x, y, barW, len, 4, val > 0 ? "top" : "bottom"), s.id);
                        })}
                        <rect
                          x={horizontal ? -M.left : b - gap}
                          y={horizontal ? b - gap : 0}
                          width={horizontal ? size.width : band.step()}
                          height={horizontal ? band.step() : h}
                          fill="transparent"
                          onPointerMove={(e) => {
                            if (quiet) return;
                            const r = (e.currentTarget.ownerSVGElement ?? e.currentTarget).getBoundingClientRect();
                            setHover({ i: ci, y: Math.round(e.clientY - r.top) });
                          }}
                          onPointerLeave={() => setHover(null)}
                        />
                        {horizontal && (
                          <>
                            <text x={-12} y={center} dy="0.32em" textAnchor="end" fill="var(--k-text-secondary)" fontSize={CATEGORY_FONT} fontWeight={500}>
                              {fitLabel(c, M.left - 12, CATEGORY_FONT)}
                              <title>{c}</title>
                            </text>
                            {(stacked ? [0] : shown.map((_, si) => si)).map((si) => {
                              const raw = stacked ? total(ci) : (shown[si]?.values[ci] ?? NaN);
                              const end = stacked
                                ? raw < 0
                                  ? Math.min(zero, ...(spans[ci] ?? []).map((sp) => (sp ? Math.min(sp.from, sp.to) : zero)))
                                  : barEnd(ci)
                                : v(vals[si]?.[ci] ?? 0);
                              const y = stacked ? center : Math.round(center - groupW / 2 + si * (barW + 4) + barW / 2);
                              return (
                                <text
                                  key={si}
                                  x={raw < 0 ? Math.round(end) - 10 : Math.round(Math.max(end, zero)) + 10}
                                  y={y}
                                  dy="0.32em"
                                  textAnchor={raw < 0 ? "end" : "start"}
                                  {...AXIS.label}
                                  fill="var(--k-text-secondary)"
                                >
                                  {valueLabel(ci, si)}
                                </text>
                              );
                            })}
                          </>
                        )}
                      </g>
                    );
                  })}
                {!horizontal &&
                  categories.map((c, ci) =>
                    labelled.has(ci) ? (
                      <text key={c} x={(band(c) ?? 0) + band.bandwidth() / 2} y={h + 20} textAnchor="middle" {...AXIS.label}>
                        {axisLabels?.[ci] ?? c}
                      </text>
                    ) : null,
                  )}
                {horizontal ? (
                  !empty && lo < 0 && <line x1={zero + 0.5} x2={zero + 0.5} y2={h} {...AXIS.baseline} />
                ) : (
                  <line x2={w} y1={zero + 0.5} y2={zero + 0.5} {...AXIS.baseline} />
                )}
              </g>
            </svg>
          )}
          {active !== null && size.width > 0 && (
            <ChartTooltip {...anchor(active)} width={size.width} visible heading={heading(active)} rows={rowsFor(active)} />
          )}
          <ChartLive>{keys.index !== null && !quiet ? liveLine(heading(keys.index), rowsFor(keys.index), t) : null}</ChartLive>
        </div>
      </ChartFrame>
    </div>
  );
});
BarChart.displayName = "BarChart";
