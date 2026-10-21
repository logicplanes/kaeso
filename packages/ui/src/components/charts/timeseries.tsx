import type * as React from "react";
import { forwardRef, useId, useMemo, useState, type ReactNode } from "react";
import { scaleLinear } from "./linear-scale.js";
import { area as d3area, curveMonotoneX, line as d3line, stack as d3stack, type SeriesPoint } from "d3-shape";
import { bisector, max as d3max, min as d3min } from "d3-array";
import { cn } from "../../utils/cn.js";
import { formatNumber, formatPercent, formatTimeZone } from "../../format/index.js";
import { useLocale, useUiText } from "../../i18n/context.js";
import {
  AXIS,
  axisLabelIndexes,
  ChartFrame,
  ChartLive,
  ChartTooltip,
  Legend,
  placeTimeLabels,
  liveLine,
  seriesColors,
  readableTicks,
  tickLabel,
  useChartKeys,
  useSize,
  useTweenedScale,
  wholeTicks,
  type ChartState,
} from "./core.js";
import { wallClock, wallDay, zoneTicks, type WallClock } from "./time-axis.js";

export const KAESO_TIMESERIES_CHART_VARIANTS = {
  kind: {
    line: { classes: "", description: "2px lines, hover dots only (default)" },
    area: { classes: "", description: "Lines with a 10% fill (16% in dark mode)" },
    stacked: { classes: "", description: "Stacked areas" },
  },
} as const;
export const KAESO_TIMESERIES_CHART_DEFAULT_VARIANTS = { kind: "line" } as const;

export interface TimePoint {
  t: Date | number | string;
  v: number;
}
export interface TimeSeries {
  id: string;
  label: string;
  points: TimePoint[];
  color?: string | undefined;
}

export interface TimeseriesChartProps extends ChartState {
  series: TimeSeries[];
  kind?: keyof typeof KAESO_TIMESERIES_CHART_VARIANTS.kind | undefined;
  height?: number | undefined;
  formatValue?: ((v: number) => string) | undefined;
  threshold?: { value: number; label: string } | undefined;
  hideLegend?: boolean | undefined;
  highlight?: number | undefined;
  formatTime?: ((t: Date) => string) | undefined;
  formatTick?: ((t: Date) => string) | undefined;
  tickAt?: "time" | "points" | undefined;
  timeZone?: string | undefined;
  shares?: boolean | undefined;
  partial?: { first?: boolean | undefined; last?: boolean | undefined; ongoing?: boolean | undefined } | undefined;
  "aria-label": string;
  className?: string | undefined;
}

const M = { top: 8, right: 52, bottom: 28, left: 0 };

function timeFormat(locale: string, options: Intl.DateTimeFormatOptions, timeZone: string | undefined): (d: Date) => string {
  const f = new Intl.DateTimeFormat(locale === "en" ? "en-US" : locale, { ...options, ...(timeZone ? { timeZone } : {}) });
  return (d) => f.format(d);
}
const CLOCK = { hour: "2-digit", minute: "2-digit", hourCycle: "h23" } as const;
const DAY = { month: "short", day: "numeric" } as const;
const H = 3_600_000;

function tickFormat(locale: string, span: number, timeZone: string | undefined, clock: WallClock) {
  if (span <= 48 * H) return timeFormat(locale, CLOCK, timeZone);
  if (span <= 90 * 24 * H) return timeFormat(locale, DAY, timeZone);
  const month = timeFormat(locale, { month: "short" }, timeZone);
  return (d: Date) => {
    const wall = new Date(clock(d));
    return wall.getUTCMonth() === 0 ? String(wall.getUTCFullYear()) : month(d);
  };
}

type PlacedLabel = { key: number; x: number; anchor: "start" | "middle" | "end"; label: string };

function timeLabels(t0: Date, t1: Date, w: number, right: number, at: (d: Date) => number, label: (d: Date) => string, clock: WallClock): PlacedLabel[] {
  let fallback: PlacedLabel[] = [];
  for (let count = Math.max(2, Math.min(10, Math.floor(w / 64))); count >= 1; count--) {
    const ticks = zoneTicks(t0, t1, count, clock);
    const placed = placeTimeLabels(
      ticks.map((d) => ({ key: d.getTime(), at: Math.round(at(d)), label: label(d) })),
      right,
    );
    if (placed.length === ticks.length && placed.length >= 2) return placed;
    if (placed.length > fallback.length) fallback = placed;
  }
  return fallback;
}

export const TimeseriesChart = forwardRef<HTMLDivElement, TimeseriesChartProps>(function TimeseriesChart(
  {
    series,
    kind = "line",
    height = 240,
    formatValue = (v) => formatNumber(v),
    threshold,
    hideLegend,
    highlight,
    formatTime,
    formatTick,
    tickAt = "time",
    timeZone,
    shares = true,
    partial,
    className,
    "aria-label": ariaLabel,
    ...state
  },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const [box, size] = useSize<HTMLDivElement>();
  const gid = useId().replace(/:/g, "");
  const axisLabel = tickLabel(formatValue);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [focus, setFocus] = useState<string | null>(null);
  const [hover, setHover] = useState<{ i: number; y: number } | null>(null);
  const visible = series.filter((s) => !hidden.has(s.id));
  const times = useMemo(() => (series[0]?.points ?? []).map((p) => new Date(p.t)), [series]);
  const flatValues = visible.flatMap((s) => s.points.map((p) => p.v));
  const w = Math.max(0, size.width - M.left - M.right);
  const h = Math.max(0, height - M.top - M.bottom);
  const stacked = kind === "stacked";
  const top = stacked ? (d3max(times.map((_, ti) => visible.reduce((a, s) => a + (s.points[ti]?.v ?? 0), 0))) ?? 0) : (d3max(flatValues) ?? 0);
  const axis = scaleLinear()
    .domain([0, Math.max(top, threshold?.value ?? 0) * 1.08 || 1])
    .nice(4)
    .range([h, 0]);
  const tween = useTweenedScale(flatValues, axis.domain()[1] ?? 1, !!state.loading);
  const values = useMemo(() => {
    let k = 0;
    return visible.map((s) => s.points.map(() => tween.values[k++] ?? 0));
  }, [visible, tween.values]);
  const yScale = axis.copy().domain([0, tween.top || 1]);
  const t0 = d3min(times) ?? new Date();
  const t1 = d3max(times) ?? new Date();
  const ordinal = tickAt === "points" && times.length > 0;
  const slot = ordinal ? w / times.length : 0;
  const x = scaleLinear().domain([t0.getTime(), t1.getTime()]).range([0, w]);
  const at = (i: number) => (ordinal ? (i + 0.5) * slot : x(times[i] ?? t0));
  const stackData = stacked
    ? d3stack<number[]>().keys(visible.map((_, i) => String(i)))(times.map((_, ti) => visible.map((_, si) => values[si]?.[ti] ?? 0)))
    : null;
  const y = (v: number) => Math.max(0, yScale(v));
  const yTicks = (state.loading ? axis.ticks(4) : wholeTicks(readableTicks(axis, axisLabel), flatValues)).filter((t) => t <= (tween.top || 1) + 1e-9);
  const zone = timeZone ?? formatTimeZone();
  const clock = wallClock(zone);
  const span = (times[times.length - 1]?.getTime() ?? 0) - (times[0]?.getTime() ?? 0);
  const fmt = formatTick ?? tickFormat(locale, span, zone, clock);
  const xLabels: PlacedLabel[] =
    tickAt === "points"
      ? axisLabelIndexes(
          times.map(fmt),
          slot,
          { start: slot / 2, end: slot / 2 + M.right },
          times.map((d) => wallDay(d, clock)),
        ).map((i) => ({ key: times[i]!.getTime(), x: Math.round(at(i)), anchor: "middle", label: fmt(times[i]!) }))
      : timeLabels(t0, t1, w, w + M.right, (d) => x(d.getTime()), fmt, clock);
  const step = (times[1]?.getTime() ?? 0) - (times[0]?.getTime() ?? 0);
  const pointFormat =
    formatTime ??
    timeFormat(
      locale,
      step && step < 86_400_000 ? { weekday: "short", ...DAY, ...CLOCK } : span <= 90 * 86_400_000 ? { weekday: "short", ...DAY } : { ...DAY, year: "numeric" },
      zone,
    );
  const last = times.length - 1;
  const headingFormat = (i: number) => {
    const heading = pointFormat(times[i]!);
    if (partial?.ongoing && i === last) return t("chart.soFar", "{heading}, so far", { heading });
    if ((partial?.last && i === last) || (partial?.first && i === 0)) return t("chart.partial", "{heading}, partial", { heading });
    return heading;
  };
  const dashedFirst = !!partial?.first && last >= 1;
  const dashedLast = !!(partial?.last || partial?.ongoing) && last >= 1;
  const cutStart = dashedFirst ? Math.round(at(1)) : -M.left;
  const cutEnd = dashedLast ? Math.round(at(last - 1)) : w + M.right;
  const partialLine = (stroke: React.SVGProps<SVGPathElement>) => {
    if (!dashedFirst && !dashedLast) return <path {...stroke} />;
    return (
      <>
        <path {...stroke} clipPath={`url(#${gid}-whole)`} />
        <path {...stroke} clipPath={`url(#${gid}-partial)`} strokeDasharray="4 4" data-partial="" />
      </>
    );
  };
  const lineGen = d3line<number>()
    .x((_, i) => Math.round(at(i)))
    .y((v) => y(v))
    .curve(curveMonotoneX);
  const areaGen = d3area<number>()
    .x((_, i) => Math.round(at(i)))
    .y0(h)
    .y1((v) => y(v))
    .curve(curveMonotoneX);
  const stackArea = d3area<SeriesPoint<number[]>>()
    .x((_, i) => Math.round(at(i)))
    .y0((d) => y(d[0]))
    .y1((d) => y(d[1]))
    .curve(curveMonotoneX);

  const stackTop = d3line<SeriesPoint<number[]>>()
    .x((_, i) => Math.round(at(i)))
    .y((d) => y(d[1]))
    .curve(curveMonotoneX);
  const palette = useMemo(() => seriesColors(series.map((s) => s.id)), [series]);
  const color = (s: TimeSeries) => s.color ?? palette.get(s.id)!;
  const bis = bisector((d: Date) => d.getTime()).center;
  const onMove = (e: React.PointerEvent<SVGRectElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const px = e.clientX - r.left;
    const i = ordinal ? Math.max(0, Math.min(times.length - 1, Math.floor(px / slot))) : bis(times, x.invert(px));
    setHover({ i, y: Math.round(e.clientY - r.top + M.top) });
  };

  const table = {
    columns: [t("timeseriesChart.time", "Time"), ...series.map((s) => s.label)],
    rows: times.map((_, i) => [headingFormat(i), ...series.map((s) => formatValue(s.points[i]?.v ?? 0))]),
  };

  const empty = state.empty ?? series.every((s) => s.points.every((p) => p.v === 0));
  const blank = !!state.loading || empty;
  const keys = useChartKeys(blank || state.error ? 0 : times.length);
  const active = blank || !!state.error ? null : (hover?.i ?? keys.index ?? (highlight !== undefined && highlight < times.length ? highlight : null));
  const activeX = active === null ? 0 : Math.round(at(active)) + M.left;
  const activeY =
    active === null
      ? 0
      : hover && hover.i === active
        ? hover.y
        : M.top + Math.round(y(stacked ? (stackData?.[stackData.length - 1]?.[active]?.[1] ?? 0) : Math.max(0, ...values.map((sv) => sv[active] ?? 0))));
  const rowsFor = (i: number) =>
    visible.map((s) => {
      const v = s.points[i]?.v ?? 0;
      const total = visible.reduce((acc, ss) => acc + (ss.points[i]?.v ?? 0), 0);
      return {
        id: s.id,
        label: s.label,
        color: color(s),
        value: formatValue(v),
        ...(shares && visible.length > 1 && total ? { share: formatPercent(Math.round((v / total) * 100) / 100, 0) } : {}),
      } as { id: string; label: ReactNode; color: string; value: ReactNode; share?: ReactNode | undefined };
    });

  return (
    <div ref={ref} className={cn("min-w-0", className)}>
      <ChartFrame
        label={ariaLabel}
        height={height}
        table={table}
        state={{ ...state, empty }}
        legend={
          !hideLegend && series.length > 1 ? (
            <Legend
              items={series.map((s) => ({ id: s.id, label: s.label, color: color(s) }))}
              hidden={hidden}
              shape={kind === "line" ? "line" : "square"}
              onToggle={(id) => {
                const n = new Set(hidden);
                if (n.has(id)) n.delete(id);
                else n.add(id);
                setHidden(n);
              }}
              onHover={setFocus}
            />
          ) : undefined
        }
      >
        <div ref={box} role="group" aria-label={ariaLabel} {...keys.props} className="absolute inset-0 rounded-cell focus-ring">
          {size.width > 0 && (
            <svg width={size.width} height={height} className="block overflow-visible" role="presentation">
              <defs>
                {(dashedFirst || dashedLast) && (
                  <>
                    <clipPath id={`${gid}-whole`}>
                      <rect x={cutStart} y={-M.top} width={Math.max(0, cutEnd - cutStart)} height={height} />
                    </clipPath>
                    <clipPath id={`${gid}-partial`}>
                      {dashedFirst && <rect x={-M.left} y={-M.top} width={cutStart + M.left} height={height} />}
                      {dashedLast && <rect x={cutEnd} y={-M.top} width={w + M.right - cutEnd} height={height} />}
                    </clipPath>
                  </>
                )}
                {visible.map((s) => (
                  <linearGradient key={s.id} id={`${gid}-${s.id}`} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={color(s)} className="chart-fill-top" />
                    <stop offset="100%" stopColor={color(s)} stopOpacity={0} />
                  </linearGradient>
                ))}
              </defs>
              <g transform={`translate(${M.left},${M.top})`}>
                {yTicks.map((t) => (
                  <g key={t} transform={`translate(0,${Math.round(y(t)) + 0.5})`}>
                    {t !== 0 && <line x2={w} {...AXIS.grid} />}
                    {!state.loading && (
                      <text x={w + 12} dy="0.32em" {...AXIS.label}>
                        {axisLabel(t)}
                      </text>
                    )}
                  </g>
                ))}
                <line x2={w} y1={h + 0.5} y2={h + 0.5} {...AXIS.baseline} />
                {xLabels.map((l) => (
                  <text key={l.key} x={l.x} y={h + 20} textAnchor={l.anchor} {...AXIS.label}>
                    {l.label}
                  </text>
                ))}
                {blank
                  ? null
                  : stacked && stackData
                    ? stackData.map((layer, li) => {
                        const s = visible[li]!;
                        return (
                          <g key={s.id} opacity={focus && focus !== s.id ? 0.3 : 1} className="opacity-motion">
                            <path d={stackArea(layer) ?? ""} fill={color(s)} className="chart-stack-fill" />
                            {partialLine({
                              d: stackTop(layer) ?? "",
                              fill: "none",
                              stroke: color(s),
                              strokeWidth: 1.5,
                              strokeLinejoin: "round",
                              strokeLinecap: "round",
                            })}
                          </g>
                        );
                      })
                    : visible.map((s, si) => (
                        <g key={s.id} opacity={focus && focus !== s.id ? 0.3 : 1} className="opacity-motion">
                          {kind === "area" && <path d={areaGen(values[si] ?? []) ?? ""} fill={`url(#${gid}-${s.id})`} />}
                          {partialLine({
                            d: lineGen(values[si] ?? []) ?? "",
                            fill: "none",
                            stroke: color(s),
                            strokeWidth: 2,
                            strokeLinejoin: "round",
                            strokeLinecap: "round",
                          })}
                        </g>
                      ))}
                {threshold && (
                  <g transform={`translate(0,${Math.round(y(threshold.value)) + 0.5})`}>
                    <line x2={w} stroke="var(--k-danger)" strokeDasharray="4 4" strokeWidth={1} />
                    <text x={0} dy={-6} fill="var(--k-danger-text)" fontSize={12} fontWeight={500}>
                      {threshold.label}
                    </text>
                  </g>
                )}
                {active !== null && (
                  <g aria-hidden>
                    <line x1={activeX - M.left + 0.5} x2={activeX - M.left + 0.5} y2={h} {...AXIS.grid} stroke="var(--k-line-strong)" />
                    {!stacked &&
                      visible.map((s, si) => (
                        <circle
                          key={s.id}
                          cx={activeX - M.left}
                          cy={y(values[si]?.[active] ?? 0)}
                          r={4}
                          fill={color(s)}
                          stroke="var(--k-bg-panel)"
                          strokeWidth={2}
                        />
                      ))}
                  </g>
                )}
                <rect width={w} height={h} fill="transparent" onPointerMove={onMove} onPointerLeave={() => setHover(null)} />
              </g>
            </svg>
          )}
          {active !== null && times[active] && (
            <ChartTooltip x={activeX} y={activeY} width={size.width} visible heading={headingFormat(active)} rows={rowsFor(active)} />
          )}
          <ChartLive>
            {keys.index !== null && active !== null && times[keys.index] ? liveLine(headingFormat(keys.index), rowsFor(keys.index), t) : null}
          </ChartLive>
        </div>
      </ChartFrame>
    </div>
  );
});
TimeseriesChart.displayName = "TimeseriesChart";
