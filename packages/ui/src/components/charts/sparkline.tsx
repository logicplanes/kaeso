import { forwardRef, useId } from "react";
import { area as d3area, curveMonotoneX, line as d3line } from "d3-shape";
import { max as d3max, min as d3min } from "d3-array";
import { cn } from "../../utils/cn.js";

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
  { values: number[]; color?: string | undefined; fluid?: boolean | undefined; "aria-label": string; className?: string | undefined }
>(function Sparkline({ values, color = "var(--k-viz-1)", fluid, className, "aria-label": ariaLabel }, ref) {
  const W = fluid ? 240 : 80;
  const H = fluid ? 32 : 24;
  const x = linear(0, Math.max(1, values.length - 1), 2, W - 4);
  const lo = d3min(values) ?? 0;
  const hi = d3max(values) ?? 1;
  const y = linear(lo, hi === lo ? lo + 1 : hi, H - 3, 3);
  const line = d3line<number>()
    .x((_, i) => x(i))
    .y((v) => y(v))
    .curve(curveMonotoneX);
  const area = d3area<number>()
    .x((_, i) => x(i))
    .y0(H)
    .y1((v) => y(v))
    .curve(curveMonotoneX);
  const last = values[values.length - 1] ?? 0;
  const gid = useId().replace(/:/g, "");
  return (
    <svg
      ref={ref}
      width={fluid ? "100%" : W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      {...(fluid ? { preserveAspectRatio: "none" } : {})}
      role="img"
      aria-label={ariaLabel}
      className={cn("block shrink-0 overflow-visible", className)}
    >
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} className="chart-fill-top" />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <path d={area(values) ?? ""} fill={`url(#${gid})`} />
      <path
        d={line(values) ?? ""}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
      {!fluid && <circle cx={x(values.length - 1)} cy={y(last)} r={2.5} fill={color} stroke="var(--k-bg-panel)" strokeWidth={1.5} paintOrder="stroke" />}
    </svg>
  );
});
Sparkline.displayName = "Sparkline";
