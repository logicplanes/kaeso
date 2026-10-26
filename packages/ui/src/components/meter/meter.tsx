import { forwardRef, type CSSProperties, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";
import { TruncatedText } from "../text/truncated.js";

export const KAESO_METER_VARIANTS = {
  tone: {
    auto: {
      classes: "",
      description:
        "Series 1 until the first alert threshold, amber (series 2) after, danger at 100%; the value and the caption take the warning or danger color too (default)",
    },
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
  const state = meterState(value, thresholds);
  return state === "over" ? "bg-danger" : state === "near" ? "bg-viz-2" : "bg-viz-1";
}

function meterState(value: number, thresholds: number[]): "over" | "near" | "under" {
  if (value >= 1) return "over";
  const first = thresholds.length ? Math.min(...thresholds) : 0.8;
  return value >= first ? "near" : "under";
}

const STATE_TEXT = { over: "text-danger-text", near: "text-warning-text", under: "" } as const;

// Each threshold is a 2px gap cut out of the bar, so the tick shows whatever surface the meter sits on.
function thresholdMask(thresholds: number[]): CSSProperties | undefined {
  if (!thresholds.length) return undefined;
  const stops = [...thresholds]
    .sort((a, b) => a - b)
    .map((t) => `black calc(${t * 100}% - 1px), transparent calc(${t * 100}% - 1px), transparent calc(${t * 100}% + 1px), black calc(${t * 100}% + 1px)`);
  const mask = `linear-gradient(to right, black 0, ${stops.join(", ")}, black 100%)`;
  return { maskImage: mask, WebkitMaskImage: mask };
}

export const Meter = forwardRef<HTMLDivElement, MeterProps>(function Meter(
  { value, label, valueLabel, caption, thresholds = [], tone = "auto", leading, trailing, className, "aria-label": ariaLabel },
  ref,
) {
  const t = useUiText();
  const pct = Math.max(0, Math.min(1, value));
  const now = Math.round(value * 100);
  const state = tone === "auto" ? meterState(value, thresholds) : "under";
  const stateWord = state === "over" ? t("meter.overLimit", "over the limit") : state === "near" ? t("meter.nearLimit", "near the limit") : "";
  const valueText = typeof valueLabel === "string" ? valueLabel : `${now}%`;
  return (
    <div ref={ref} className={cn("flex flex-col gap-8", className)}>
      {(label || valueLabel) && (
        <div className="flex items-baseline justify-between gap-12">
          <span className="flex min-w-0 items-center gap-8 text-body font-medium text-fg">
            {leading}
            {label && <TruncatedText className="min-w-0">{label}</TruncatedText>}
            {trailing}
          </span>
          {valueLabel && <span className={cn("shrink-0 text-body tabular", STATE_TEXT[state] || "text-fg-secondary")}>{valueLabel}</span>}
        </div>
      )}
      <div
        role="meter"
        aria-valuemin={0}
        aria-valuemax={Math.max(100, now)}
        aria-valuenow={now}
        aria-label={ariaLabel ?? (typeof label === "string" ? label : undefined)}
        {...(typeof valueLabel === "string" || stateWord ? { "aria-valuetext": stateWord ? `${valueText}, ${stateWord}` : valueText } : {})}
        style={thresholdMask(thresholds)}
        className="relative h-6 overflow-hidden rounded-pill bg-fill-2 forced-colors:outline forced-colors:-outline-offset-1"
      >
        <div
          className={cn(
            "h-full rounded-pill width-motion data-mark",
            tone === "auto" ? meterColor(value, thresholds) : KAESO_METER_VARIANTS.tone.neutral.classes,
          )}
          style={{ width: `${pct * 100}%` }}
        />
      </div>
      {caption && <span className={cn("text-caption font-medium", STATE_TEXT[state] || "text-fg-secondary")}>{caption}</span>}
    </div>
  );
});
Meter.displayName = "Meter";

export const KAESO_PROGRESS_VARIANTS = {
  kind: {
    determinate: { classes: "h-6 rounded-pill bg-fill-2 forced-colors:outline forced-colors:-outline-offset-1", description: "A pill bar with a known amount" },
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
        <span className="absolute inset-y-0 left-0 w-1/3 animate-indeterminate rounded-pill bg-fg-tertiary data-mark" />
      ) : (
        <span className="block h-full rounded-pill bg-ink width-motion forced-line" style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }} />
      )}
    </div>
  );
});
Progress.displayName = "Progress";
