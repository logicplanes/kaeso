import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";
import { RollingNumber } from "./rolling-number.js";
import { useLocale, useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_DELTA_VARIANTS = {
  tone: {
    better: { classes: "text-success-text", description: "The change goes the way people want" },
    worse: { classes: "text-danger-text", description: "The change goes the wrong way" },
    flat: { classes: "text-fg-secondary", description: "No change, or a change with no better side" },
  },
} as const;
export const KAESO_DELTA_DEFAULT_VARIANTS = {} as const;

export function deltaLabel(v: number, t: UiText, locale: string): string {
  const pct = Math.abs(v) * 100;
  const digits = pct >= 10 || pct === 0 ? 0 : 1;
  const shown = new Intl.NumberFormat(locale === "en" ? "en-US" : locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(digits ? Number(pct.toFixed(1)) : Math.round(pct));
  return `${v > 0 ? "↑" : v < 0 ? "↓" : ""}${t("statTile.percent", "{value}%", { value: shown })}`;
}

export interface DeltaProps {
  /** The change as a share: 0.16 for 16% more, -0.04 for 4% less. */
  value: number;
  /** Which direction is good; without it the change reads as neutral. */
  better?: "up" | "down" | undefined;
  className?: string | undefined;
}

/** A change written as text, "↑16%", in green or red only when one direction is better, and said in words for screen readers. */
export const Delta = forwardRef<HTMLSpanElement, DeltaProps>(function Delta({ value, better = "up", className }, ref) {
  const t = useUiText();
  const locale = useLocale();
  const good = value === 0 || !Number.isFinite(value) ? undefined : value > 0 === (better === "up");
  const tone = good === undefined ? "flat" : good ? "better" : "worse";
  return (
    <span
      ref={ref}
      className={cn("inline-flex shrink-0 items-center text-small font-medium whitespace-nowrap tabular", KAESO_DELTA_VARIANTS.tone[tone].classes, className)}
    >
      <RollingNumber value={deltaLabel(value, t, locale)} />
      {good !== undefined && <span className="sr-only"> {good ? t("statTile.better", "better") : t("statTile.worse", "worse")}</span>}
    </span>
  );
});
Delta.displayName = "Delta";
