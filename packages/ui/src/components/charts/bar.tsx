import { forwardRef, useState } from "react";
import { scaleBand, scaleLinear } from "d3-scale";
import { max as d3max } from "d3-array";
import { cn } from "../../utils/cn";
import { formatNumber, formatPercent } from "../../format";
import { useUiText } from "../../i18n/context";
import {
  AXIS,
  axisLabelIndexes,
  type AxisDate,
  ChartFrame,
  ChartLive,
  ChartTooltip,
  Legend,
  liveLine,
  seriesColors,
  readableTicks,
  tickLabel,
  useChartKeys,
  useSize,
  useTweenedScale,
  wholeTicks,
  type ChartState,
} from "./core";

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
  "aria-label": string;
  className?: string | undefined;
}

export const BarChart = forwardRef<HTMLDivElement, BarChartProps>(function BarChart(
  {
    categories,
    series,
    layout = "grouped",
    height = 240,
    formatValue = (v) => formatNumber(v),
    highlight,
    axisLabels,
    dates,
    className,
    "aria-label": ariaLabel,
    ...state
  },
  ref,
) {
  const t = useUiText();
  const [box, size] = useSize<HTMLDivElement>();
  const [hover, setHover] = useState<{ i: number; y: number } | null>(null);
  const [focus, setFocus] = useState<string | null>(null);
  const horizontal = layout === "horizontal";
  const stacked = layout === "stacked";
  const flat = series.flatMap((s) => s.values);
  const dataTop = (stacked ? d3max(categories.map((_, ci) => series.reduce((a, s) => a + (s.values[ci] ?? 0), 0))) : d3max(flat)) ?? 0;
  const axis = scaleLinear()
    .domain([0, dataTop * 1.08 || 1])
    .nice(4);
  const tween = useTweenedScale(flat, axis.domain()[1] ?? 1, !!state.loading);
  const vals = series.map((s, si) => s.values.map((_, ci) => tween.values[si * s.values.length + ci] ?? 0));
  const palette = seriesColors(series.map((s) => s.id));
  const color = (s: BarSeries) => s.color ?? palette.get(s.id)!;
  const M = horizontal ? { top: 0, right: 56, bottom: 0, left: 140 } : { top: 8, right: 52, bottom: 28, left: 0 };
  const w = Math.max(0, size.width - M.left - M.right);
  const h = Math.max(0, height - M.top - M.bottom);
  const totals = categories.map((_, ci) => vals.reduce((a, v) => a + (v[ci] ?? 0), 0));
  const band = scaleBand<string>()
    .domain(categories)
    .range(horizontal ? [0, h] : [0, w])
    .paddingInner(0.24)
    .paddingOuter(0.12);
  axis.range(horizontal ? [0, w] : [h, 0]);
  const scale = axis.copy().domain([0, tween.top || 1]);
  const v = (n: number) => (horizontal ? Math.min(w, scale(n)) : Math.max(0, scale(n)));
  const inner = scaleBand<string>()
    .domain(series.map((s) => s.id))
    .range([0, band.bandwidth()])
    .paddingInner(0.12);
  const MAX = horizontal ? 20 : stacked || series.length === 1 ? 40 : 20;
  const barW = Math.max(4, Math.min(MAX, Math.round(stacked || horizontal ? band.bandwidth() : inner.bandwidth())));
  const groupW = stacked || horizontal ? barW : series.length * barW + (series.length - 1) * 4;
  const axisLabel = tickLabel(formatValue);
  const centers = [categories[0], categories[categories.length - 1]].map((c) => (c === undefined ? 0 : (band(c) ?? 0) + band.bandwidth() / 2));
  const shown = new Set(
    horizontal
      ? []
      : axisLabelIndexes(
          categories.map((c, ci) => axisLabels?.[ci] ?? c),
          band.step(),
          { start: M.left + centers[0]!, end: w - centers[1]! + M.right },
          dates,
        ),
  );
  const GAP = 2;
  const table = {
    columns: [t("barChart.category", "Category"), ...series.map((s) => s.label)],
    rows: categories.map((c, ci) => [c, ...series.map((s) => formatValue(s.values[ci] ?? 0))]),
  };
  const ticks = (state.loading ? axis.ticks(4) : wholeTicks(readableTicks(axis, axisLabel), flat)).filter((t) => t <= (tween.top || 1) + 1e-9);
  const quiet = !!state.loading || !!state.empty || !!state.error;
  const keys = useChartKeys(quiet ? 0 : categories.length, horizontal ? { vertical: true, start: 0 } : {});
  const active = quiet ? null : (hover?.i ?? keys.index ?? (highlight !== undefined && highlight < categories.length ? highlight : null));
  const anchor = (ci: number) => {
    const b = Math.round(band(categories[ci]!) ?? 0);
    if (horizontal) {
      const rowTop = M.top + Math.round(b + band.bandwidth() / 2 - barW / 2 - PAD);
      const rowBottom = rowTop + barW + PAD * 2;
      const below = ci < categories.length / 2;
      const tall = 40 + series.length * 22;
      return { x: M.left - 16, y: below ? rowBottom + 24 + 4 : rowTop - tall + 24 - 4 };
    }
    const lit = Math.min(band.bandwidth(), groupW + PAD * 2);
    const center = M.left + b + band.bandwidth() / 2;
    const tallest = stacked ? (totals[ci] ?? 0) : Math.max(0, ...vals.map((sv) => sv[ci] ?? 0));
    return {
      x: Math.round(center + lit / 2) - 8,
      xLeft: Math.round(center - lit / 2) + 8,
      y: hover && hover.i === ci ? hover.y : M.top + Math.round(v(tallest)),
    };
  };
  const rowsFor = (ci: number) =>
    series.map((s) => ({
      id: s.id,
      label: s.label,
      color: color(s),
      value: formatValue(s.values[ci] ?? 0),
      ...(series.length > 1 && totals[ci] ? { share: formatPercent(Math.round(((s.values[ci] ?? 0) / (totals[ci] ?? 1)) * 100) / 100, 0) } : {}),
    }));
  const PAD = 8;
  return (
    <div ref={ref} className={cn("min-w-0", className)}>
      <ChartFrame
        label={ariaLabel}
        height={height}
        table={table}
        state={state}
        legend={series.length > 1 ? <Legend items={series.map((s) => ({ id: s.id, label: s.label, color: color(s) }))} onHover={setFocus} /> : undefined}
      >
        <div ref={box} role="group" aria-label={ariaLabel} {...keys.props} className="absolute inset-0 rounded-cell focus-ring">
          {size.width > 0 && (
            <svg width={size.width} height={height} className="block overflow-visible" role="presentation">
              <g transform={`translate(${M.left},${M.top})`}>
                {!horizontal &&
                  ticks.map((t) => (
                    <g key={t} transform={`translate(0,${Math.round(v(t)) + 0.5})`}>
                      {t !== 0 && <line x2={w} {...AXIS.grid} />}
                      {!state.loading && (
                        <text x={w + 12} dy="0.32em" {...AXIS.label}>
                          {axisLabel(t)}
                        </text>
                      )}
                    </g>
                  ))}
                {categories.map((c, ci) => {
                  const b = Math.round(band(c) ?? 0);
                  const gap = Math.round((band.step() - band.bandwidth()) / 2);
                  const lit = Math.min(band.bandwidth(), groupW + PAD * 2);
                  let acc = 0;
                  return (
                    <g key={c}>
                      <rect
                        aria-hidden
                        className="opacity-motion"
                        opacity={active === ci ? 1 : 0}
                        fill="var(--k-fill-1)"
                        rx={6}
                        {...(horizontal
                          ? { x: -M.left + PAD, y: Math.round(b + band.bandwidth() / 2 - barW / 2 - PAD), width: size.width - PAD * 2, height: barW + PAD * 2 }
                          : { x: Math.round(b + band.bandwidth() / 2 - lit / 2), y: 0, width: lit, height: h })}
                      />
                      {(() => {
                        const center = b + band.bandwidth() / 2;
                        const start = Math.round(center - groupW / 2);
                        const lastVisible = series.map((_, si) => (vals[si]?.[ci] ?? 0) > 0).lastIndexOf(true);
                        let below = horizontal ? 0 : h;
                        return series.map((s, si) => {
                          const val = vals[si]?.[ci] ?? 0;
                          const dim = focus && focus !== s.id ? 0.3 : 1;
                          if (horizontal) {
                            const x0 = stacked ? Math.round(v(acc)) : 0;
                            acc += val;
                            const len = Math.max(val > 0 ? 2 : 0, Math.round(v(stacked ? acc : val)) - x0);
                            const y = Math.round(center - barW / 2);
                            return <rect key={s.id} x={x0} y={y} width={len} height={barW} rx={Math.min(4, barW / 2)} fill={color(s)} opacity={dim} />;
                          }
                          if (stacked) {
                            acc += val;
                            const top = Math.round(v(acc));
                            const first = below === h;
                            const bottom = first ? below : below - GAP;
                            below = top;
                            const bh = bottom - top;
                            if (val <= 0 || bh <= 0) return null;
                            return <path key={s.id} d={roundedTop(start, top, barW, bh, si === lastVisible ? 4 : 0)} fill={color(s)} opacity={dim} />;
                          }
                          const top = Math.round(v(val));
                          const bh = Math.max(val > 0 ? 2 : 0, h - top);
                          return <path key={s.id} d={roundedTop(start + si * (barW + 4), h - bh, barW, bh, 4)} fill={color(s)} opacity={dim} />;
                        });
                      })()}
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
                      {horizontal ? (
                        <>
                          <text x={-12} y={b + band.bandwidth() / 2} dy="0.32em" textAnchor="end" fill="var(--k-text-secondary)" fontSize={13} fontWeight={500}>
                            {c}
                          </text>
                          <text
                            x={Math.round(v(stacked ? (totals[ci] ?? 0) : (vals[0]?.[ci] ?? 0))) + 10}
                            y={b + band.bandwidth() / 2}
                            dy="0.32em"
                            {...AXIS.label}
                            fill="var(--k-text-secondary)"
                          >
                            {formatValue(stacked ? (totals[ci] ?? 0) : (series[0]?.values[ci] ?? 0))}
                          </text>
                        </>
                      ) : (
                        shown.has(ci) && (
                          <text x={b + band.bandwidth() / 2} y={h + 20} textAnchor="middle" {...AXIS.label}>
                            {axisLabels?.[ci] ?? c}
                          </text>
                        )
                      )}
                    </g>
                  );
                })}
                {!horizontal && <line x2={w} y1={h + 0.5} y2={h + 0.5} {...AXIS.baseline} />}
              </g>
            </svg>
          )}
          {active !== null && size.width > 0 && (
            <ChartTooltip {...anchor(active)} width={size.width} visible heading={categories[active]} rows={rowsFor(active)} />
          )}
          <ChartLive>{keys.index !== null && !quiet ? liveLine(categories[keys.index], rowsFor(keys.index), t) : null}</ChartLive>
        </div>
      </ChartFrame>
    </div>
  );
});
BarChart.displayName = "BarChart";

function roundedTop(x: number, y: number, w: number, h: number, r: number) {
  const rr = Math.min(r, w / 2, h);
  if (rr <= 0) return `M${x} ${y}h${w}v${h}h${-w}z`;
  return `M${x} ${y + h}V${y + rr}Q${x} ${y} ${x + rr} ${y}H${x + w - rr}Q${x + w} ${y} ${x + w} ${y + rr}V${y + h}z`;
}
