import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn";

export const KAESO_METER_VARIANTS = {
  tone: {
    auto: { classes: "", description: "Series 1 until the first alert threshold, warning after, danger at 100% (default)" },
    neutral: { classes: "bg-fg-secondary", description: "A plain share" },
  },
} as const;
export const KAESO_METER_DEFAULT_VARIANTS = { tone: "auto" } as const;

export interface MeterProps {
  value: number;
  label?: ReactNode | undefined;
  valueLabel?: ReactNode | undefined;
  caption?: ReactNode | undefined;
  thresholds?: number[] | undefined;
  tone?: keyof typeof KAESO_METER_VARIANTS.tone | undefined;
  leading?: ReactNode | undefined;
  trailing?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

export function meterColor(value: number, thresholds: number[] = []) {
  if (value >= 1) return "bg-danger";
  const first = thresholds.length ? Math.min(...thresholds) : 0.8;
  return value >= first ? "bg-warning" : "bg-viz-1";
}

export const Meter = forwardRef<HTMLDivElement, MeterProps>(function Meter(
  { value, label, valueLabel, caption, thresholds = [], tone = "auto", leading, trailing, className, "aria-label": ariaLabel },
  ref,
) {
  const pct = Math.max(0, Math.min(1, value));
  const now = Math.round(value * 100);
  return (
    <div ref={ref} className={cn("flex flex-col gap-8", className)}>
      {(label || valueLabel) && (
        <div className="flex items-baseline justify-between gap-12">
          <span className="flex min-w-0 items-center gap-8 text-body font-medium text-fg">
            {leading}
            <span className="truncate">{label}</span>
            {trailing}
          </span>
          {valueLabel && <span className="shrink-0 text-body text-fg-secondary tabular">{valueLabel}</span>}
        </div>
      )}
      <div
        role="meter"
        aria-valuemin={0}
        aria-valuemax={Math.max(100, now)}
        aria-valuenow={now}
        aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)}
        {...(typeof valueLabel === "string" ? { "aria-valuetext": valueLabel } : {})}
        className="relative h-6 overflow-hidden rounded-pill bg-fill-3"
      >
        <div
          className={cn("h-full rounded-pill width-motion", tone === "auto" ? meterColor(value, thresholds) : KAESO_METER_VARIANTS.tone.neutral.classes)}
          style={{ width: `${pct * 100}%` }}
        />
        {thresholds.map((t) => (
          <span key={t} aria-hidden className="absolute top-0 h-full w-2 bg-panel" style={{ left: `${t * 100}%` }} />
        ))}
      </div>
      {caption && <span className="text-caption font-medium text-fg-tertiary">{caption}</span>}
    </div>
  );
});
Meter.displayName = "Meter";

export const KAESO_PROGRESS_VARIANTS = {
  kind: {
    determinate: { classes: "h-6 rounded-pill bg-fill-3", description: "A pill bar with a known amount" },
    indeterminate: { classes: "h-2 bg-transparent", description: "A 2px line at the top of a container" },
  },
} as const;
export const KAESO_PROGRESS_DEFAULT_VARIANTS = { kind: "determinate" } as const;

export interface ProgressProps {
  value?: number | undefined;
  "aria-label": string;
  className?: string | undefined;
}

export const Progress = forwardRef<HTMLDivElement, ProgressProps>(function Progress({ value, className, "aria-label": ariaLabel }, ref) {
  const indeterminate = value === undefined;
  return (
    <div
      ref={ref}
      role="progressbar"
      aria-label={ariaLabel}
      aria-valuemin={0}
      aria-valuemax={100}
      {...(!indeterminate ? { "aria-valuenow": Math.round(value * 100) } : {})}
      className={cn("relative w-full overflow-hidden", KAESO_PROGRESS_VARIANTS.kind[indeterminate ? "indeterminate" : "determinate"].classes, className)}
    >
      {indeterminate ? (
        <span className="absolute inset-y-0 left-0 w-1/3 animate-indeterminate rounded-pill bg-fg-tertiary" />
      ) : (
        <span className="block h-full rounded-pill bg-ink width-motion" style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }} />
      )}
    </div>
  );
});
Progress.displayName = "Progress";
