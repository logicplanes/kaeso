import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatPercent } from "../../format/index.js";
import { useTweened } from "./core.js";
import { useLocale } from "../../i18n/context.js";

export const KAESO_GAUGE_VARIANTS = {
  size: { md: { classes: "w-180", description: "180px (default)" } },
} as const;
export const KAESO_GAUGE_DEFAULT_VARIANTS = { size: "md" } as const;

export const Gauge = forwardRef<
  HTMLDivElement,
  {
    value: number;
    thresholds?: number[] | undefined;
    label?: ReactNode | undefined;
    valueLabel?: ReactNode | undefined;
    "aria-label": string;
    className?: string | undefined;
  }
>(function Gauge({ value, thresholds = [0.8], label, valueLabel, className, "aria-label": ariaLabel }, ref) {
  useLocale();
  const W = 180;
  const R = 82;
  const [t] = useTweened([Math.max(0, Math.min(1, value))]);
  const a = (f: number) => Math.PI * (1 - f);
  const pt = (f: number, r: number) => [W / 2 + r * Math.cos(a(f)), 94 - r * Math.sin(a(f))] as const;
  const path = (from: number, to: number) => {
    const [x1, y1] = pt(from, R);
    const [x2, y2] = pt(to, R);
    return `M${x1} ${y1}A${R} ${R} 0 0 1 ${x2} ${y2}`;
  };
  const now = Math.round(value * 100);
  const tone = value >= 1 ? "var(--k-danger)" : value >= Math.min(...thresholds, 1) ? "var(--k-warning)" : "var(--k-viz-1)";
  return (
    <div
      ref={ref}
      role="meter"
      aria-label={ariaLabel}
      aria-valuemin={0}
      aria-valuemax={Math.max(100, now)}
      aria-valuenow={now}
      className={cn("relative inline-flex flex-col items-center", className)}
    >
      <svg width={W} height={104} role="presentation">
        <path d={path(0, 1)} fill="none" stroke="var(--k-fill-3)" strokeWidth={12} strokeLinecap="round" />
        {(t ?? 0) > 0 && <path d={path(0, Math.max(0.005, t ?? 0))} fill="none" stroke={tone} strokeWidth={12} strokeLinecap="round" />}
        {thresholds.map((th) => {
          const [x1, y1] = pt(th, R - 9);
          const [x2, y2] = pt(th, R + 9);
          return <line key={th} x1={x1} y1={y1} x2={x2} y2={y2} stroke="var(--k-bg-panel)" strokeWidth={2} />;
        })}
      </svg>
      <div className="absolute inset-x-0 bottom-0 flex flex-col items-center">
        <span className="text-title font-semibold text-fg tabular">{valueLabel ?? formatPercent(value, 0)}</span>
        {label && <span className="text-caption font-medium text-fg-tertiary">{label}</span>}
      </div>
    </div>
  );
});
Gauge.displayName = "Gauge";
