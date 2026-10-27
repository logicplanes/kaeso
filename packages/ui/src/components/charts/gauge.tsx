import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatPercent } from "../../format/index.js";
import { useTweened } from "./core.js";
import { useUiText } from "../../i18n/context.js";

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
  const t = useUiText();
  const W = 180;
  const R = 82;
  const known = Number.isFinite(value);
  const [drawn] = useTweened([known ? Math.max(0, Math.min(1, value)) : 0]);
  const a = (f: number) => Math.PI * (1 - f);
  const pt = (f: number, r: number) => [W / 2 + r * Math.cos(a(f)), 94 - r * Math.sin(a(f))] as const;
  const path = (from: number, to: number) => {
    const [x1, y1] = pt(from, R);
    const [x2, y2] = pt(to, R);
    return `M${x1} ${y1}A${R} ${R} 0 0 1 ${x2} ${y2}`;
  };
  const now = known ? Math.round(value * 100) : 0;
  const over = known && value >= 1;
  const near = known && !over && value >= Math.min(...thresholds, 1);
  const tone = over ? "var(--k-danger)" : near ? "var(--k-warning)" : "var(--k-viz-1)";
  const status = over ? t("gauge.over", "Over the limit") : near ? t("gauge.near", "Near the limit") : null;
  const shown = valueLabel ?? (known ? formatPercent(value, 0) : "–");
  const spokenValue = typeof valueLabel === "string" ? valueLabel : known ? formatPercent(value, 0) : t("gauge.noData", "No data");
  const spoken = [spokenValue, typeof label === "string" ? label : null, status].filter(Boolean).join(", ");
  return (
    <div
      ref={ref}
      role="meter"
      aria-label={ariaLabel}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.max(0, Math.min(100, now))}
      aria-valuetext={spoken}
      className={cn("relative inline-flex w-180 flex-col items-center", className)}
    >
      <div className="relative">
        <svg width={W} height={104} role="presentation" className="block data-mark">
          <path d={path(0, 1)} fill="none" stroke="var(--k-fill-3)" strokeWidth={12} strokeLinecap="round" />
          {(drawn ?? 0) > 0 && <path d={path(0, Math.max(0.005, drawn ?? 0))} fill="none" stroke={tone} strokeWidth={12} strokeLinecap="round" />}
          {thresholds.map((th) => {
            const [x1, y1] = pt(th, R - 6);
            const [x2, y2] = pt(th, R + 10);
            return <line key={th} x1={x1} y1={y1} x2={x2} y2={y2} stroke="var(--k-text-tertiary)" strokeWidth={2} strokeLinecap="round" />;
          })}
        </svg>
        <div className="absolute inset-x-0 bottom-0 mx-auto flex max-w-140 flex-col items-center text-center">
          {status && <span className={cn("text-caption font-medium", over ? "text-danger-text" : "text-warning-text")}>{status}</span>}
          <span className="max-w-full text-title font-semibold text-fg tabular wrap-anywhere">{shown}</span>
        </div>
      </div>
      {label && <span className="mt-4 max-w-full text-center text-caption font-medium text-pretty text-fg-tertiary">{label}</span>}
    </div>
  );
});
Gauge.displayName = "Gauge";
