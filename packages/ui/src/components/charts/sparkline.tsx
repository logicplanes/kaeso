import { forwardRef, useId, useState, type KeyboardEvent } from "react";
import { area as d3area, curveMonotoneX, line as d3line } from "d3-shape";
import { max as d3max, min as d3min } from "d3-array";
import { cn } from "../../utils/cn.js";
import { ChartTooltip, useSize } from "./tooltip.js";

function linear(d0: number, d1: number, r0: number, r1: number): (v: number) => number {
  const span = d1 - d0;
  return (v) => {
    const t = span ? (v - d0) / span : Number.isNaN(span) ? NaN : 0.5;
    return r0 * (1 - t) + r1 * t;
  };
}

export const KAESO_SPARKLINE_VARIANTS = {
  size: { md: { classes: "w-80 h-24", description: "80×24 inline (default)" } },
} as const;
export const KAESO_SPARKLINE_DEFAULT_VARIANTS = { size: "md" } as const;

export const Sparkline = forwardRef<
  SVGSVGElement,
  {
    values: number[];
    color?: string | undefined;
    fluid?: boolean | undefined;
    height?: number | undefined;
    labels?: string[] | undefined;
    seriesLabel?: string | undefined;
    formatValue?: ((v: number) => string) | undefined;
    "aria-label": string;
    className?: string | undefined;
  }
>(function Sparkline({ values, color = "var(--k-viz-1)", fluid, height, labels, seriesLabel, formatValue = String, className, "aria-label": ariaLabel }, ref) {
  const W = fluid ? 240 : 80;
  const H = fluid ? (height ?? 32) : 24;
  const [box, size] = useSize<HTMLSpanElement>();
  const [at, setAt] = useState<number | null>(null);
  const points = values.filter(Number.isFinite);
  const x = linear(0, Math.max(1, values.length - 1), 2, W - 4);
  const lo = d3min(points) ?? 0;
  const hi = d3max(points) ?? 0;
  const y = hi === lo ? () => H / 2 : linear(lo, hi, H - 3, 3);
  const line = d3line<number>()
    .defined(Number.isFinite)
    .x((_, i) => x(i))
    .y((v) => y(v))
    .curve(curveMonotoneX);
  const area = d3area<number>()
    .defined(Number.isFinite)
    .x((_, i) => x(i))
    .y0(H)
    .y1((v) => y(v))
    .curve(curveMonotoneX);
  const lastIndex = values.findLastIndex(Number.isFinite);
  const single = points.length === 1;
  const dotX = single ? W - 4 : x(lastIndex);
  const gid = useId().replace(/:/g, "");
  const art = (
    <svg
      ref={ref}
      width={fluid ? "100%" : W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      {...(fluid ? { preserveAspectRatio: "none" } : {})}
      {...(labels ? { "aria-hidden": true } : { role: "img", "aria-label": ariaLabel })}
      className={cn("block shrink-0 overflow-visible", className)}
    >
      {points.length === 0 ? (
        <line x1={2} x2={W - 4} y1={H / 2} y2={H / 2} stroke="var(--k-line-strong)" strokeWidth={1.5} strokeLinecap="round" vectorEffect="non-scaling-stroke" />
      ) : (
        <>
          <defs>
            <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} className="chart-fill-top" />
              <stop offset="100%" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          {!single && <path d={area(values) ?? ""} fill={`url(#${gid})`} />}
          {!single && (
            <path
              d={line(values) ?? ""}
              fill="none"
              stroke={color}
              strokeWidth={1.5}
              strokeLinejoin="round"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
          )}
          {at !== null && Number.isFinite(values[at]) && (
            <g aria-hidden data-crosshair="">
              <line x1={x(at)} x2={x(at)} y1={0} y2={H} stroke="var(--k-line-strong)" strokeWidth={1} vectorEffect="non-scaling-stroke" />
              <path
                d={`M${x(at)} ${y(values[at] ?? 0)}h0`}
                stroke="var(--k-bg-panel)"
                strokeWidth={9}
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
              <path d={`M${x(at)} ${y(values[at] ?? 0)}h0`} stroke={color} strokeWidth={6} strokeLinecap="round" vectorEffect="non-scaling-stroke" />
            </g>
          )}
          {(!fluid || single) && at === null && (
            <g data-dot="">
              <path
                d={`M${dotX} ${y(values[lastIndex] ?? 0)}h0`}
                stroke="var(--k-bg-panel)"
                strokeWidth={8}
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
              <path d={`M${dotX} ${y(values[lastIndex] ?? 0)}h0`} stroke={color} strokeWidth={5} strokeLinecap="round" vectorEffect="non-scaling-stroke" />
            </g>
          )}
        </>
      )}
    </svg>
  );
  if (!labels) return art;
  const last = values.length - 1;
  const pick = (clientX: number, el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    const ratio = r.width ? (clientX - r.left) / r.width : 0;
    setAt(Math.max(0, Math.min(last, Math.round(ratio * last))));
  };
  const onKey = (e: KeyboardEvent) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (e.key === "Escape") return setAt(null);
    if (!step && e.key !== "Home" && e.key !== "End") return;
    e.preventDefault();
    setAt((i) => (e.key === "Home" ? 0 : e.key === "End" ? last : Math.max(0, Math.min(last, (i ?? last) + step))));
  };
  const px = at === null ? 0 : (x(at) / W) * size.width;
  return (
    <span
      ref={box}
      tabIndex={0}
      role="group"
      aria-label={ariaLabel}
      onPointerMove={(e) => pick(e.clientX, e.currentTarget)}
      onPointerLeave={() => setAt(null)}
      onBlur={() => setAt(null)}
      onKeyDown={onKey}
      className="relative block w-full cursor-crosshair rounded-cell focus-ring"
    >
      {art}
      {at !== null && (
        <ChartTooltip
          x={px}
          y={0}
          width={size.width}
          visible
          heading={labels[at] ?? ""}
          rows={[{ id: "v", label: seriesLabel ?? ariaLabel, color, value: Number.isFinite(values[at]) ? formatValue(values[at] ?? 0) : "–" }]}
        />
      )}
      <span className="sr-only" aria-live="polite">
        {at !== null ? `${labels[at] ?? ""}: ${Number.isFinite(values[at]) ? formatValue(values[at] ?? 0) : "–"}` : ""}
      </span>
    </span>
  );
});
Sparkline.displayName = "Sparkline";
