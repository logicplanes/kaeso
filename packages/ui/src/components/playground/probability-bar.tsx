import { forwardRef, useId, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";
import type { Formatters } from "../../format/index.js";

export const KAESO_PROBABILITY_BAR_VARIANTS = {
  layout: {
    inline: {
      classes: "flex min-h-22 items-center gap-12",
      description: "The label, a 6px bar and the value on one line, so a list of options lines up (default)",
    },
    stacked: { classes: "flex flex-col gap-6", description: "The label and the value above a full-width bar, for one probability alone" },
  },
} as const;
export const KAESO_PROBABILITY_BAR_DEFAULT_VARIANTS = { layout: "inline" } as const;

export interface ProbabilityBarProps {
  value?: number | null | undefined;
  label?: ReactNode | undefined;
  highlight?: boolean | undefined;
  pending?: boolean | undefined;
  layout?: keyof typeof KAESO_PROBABILITY_BAR_VARIANTS.layout | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

/** A probability as people read it: whole percents, with "<1%" and ">99%" where rounding would say 0% or 100% of something that isn't. */
export function probabilityText(p: number, t: UiText, { formatPercent }: Pick<Formatters, "formatPercent">): string {
  if (p > 0 && p < 0.005) return t("probabilityBar.below", "<{percent}", { percent: formatPercent(0.01, 0) });
  if (p < 1 && p >= 0.995) return t("probabilityBar.above", ">{percent}", { percent: formatPercent(0.99, 0) });
  return formatPercent(p, 0);
}

function known(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export const ProbabilityBar = forwardRef<HTMLDivElement, ProbabilityBarProps>(function ProbabilityBar(
  { value, label, highlight = false, pending = false, layout = "inline", "aria-label": ariaLabel, className },
  ref,
) {
  const t = useUiText();
  const formatters = useFormatters();
  const labelId = useId();
  const has = !pending && known(value);
  const p = has ? Math.max(0, Math.min(1, value)) : 0;
  const shown = has ? probabilityText(p, t, formatters) : "";
  const valueText = highlight ? t("probabilityBar.chosen", "{percent}, chosen", { percent: shown }) : shown;
  const missing = !pending && !has;
  const missingText = t("probabilityBar.missing", "Not given");
  const labelEl =
    label !== undefined && label !== null ? (
      <span
        id={labelId}
        className={cn(
          "min-w-0 text-small wrap-anywhere",
          layout === "inline" ? "w-120 shrink-0" : "flex-1",
          highlight ? "font-medium text-fg" : "text-fg-secondary",
        )}
      >
        {label}
      </span>
    ) : null;
  const valueEl = (
    <span
      className={cn(
        "shrink-0 text-small tabular",
        layout === "inline" && "w-48 text-end",
        highlight ? "font-medium text-fg" : missing ? "text-fg-tertiary" : "text-fg-secondary",
      )}
    >
      {missing && layout === "stacked" ? missingText : shown}
    </span>
  );
  const track =
    missing && layout === "inline" ? (
      <span className="min-w-0 flex-1 truncate text-small text-fg-tertiary">{missingText}</span>
    ) : (
      <span
        aria-hidden
        className={cn(
          "relative block h-6 min-w-0 overflow-hidden rounded-pill bg-fill-2 forced-colors:outline forced-colors:-outline-offset-1",
          layout === "inline" && "flex-1",
          pending && "animate-pulse-quiet reduced:animate-none",
        )}
      >
        {has && (
          <span
            className={cn("block h-full rounded-pill width-motion reduced:transition-none data-mark forced-line", highlight ? "bg-viz-1" : "bg-fg-tertiary")}
            style={{ width: `${p * 100}%` }}
          />
        )}
      </span>
    );
  const body =
    layout === "inline" ? (
      <>
        {labelEl}
        {track}
        {!missing && valueEl}
      </>
    ) : (
      <>
        <span className="flex items-baseline justify-between gap-12">
          {labelEl ?? <span className="flex-1" />}
          {valueEl}
        </span>
        {track}
      </>
    );
  const name = ariaLabel ?? (typeof label === "string" ? label : undefined);
  return (
    <div
      ref={ref}
      {...(has
        ? {
            role: "meter",
            "aria-valuemin": 0,
            "aria-valuemax": 100,
            "aria-valuenow": Math.round(p * 1000) / 10,
            "aria-valuetext": valueText,
            ...(name !== undefined ? { "aria-label": name } : labelEl ? { "aria-labelledby": labelId } : {}),
          }
        : {})}
      {...(pending ? { "aria-hidden": true } : {})}
      data-probability-bar=""
      data-highlight={highlight || undefined}
      className={cn(KAESO_PROBABILITY_BAR_VARIANTS.layout[layout].classes, "min-w-0", className)}
    >
      {body}
    </div>
  );
});
ProbabilityBar.displayName = "ProbabilityBar";
