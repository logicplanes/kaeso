import { forwardRef } from "react";
import { Slider as BaseSlider } from "@base-ui/react/slider";
import { cn } from "../../utils/cn.js";
import { useLocale, useUiText } from "../../i18n/context.js";

export const KAESO_SLIDER_VARIANTS = {
  size: {
    md: { classes: "h-32", description: "A 4px track in a 32px hit area (default)" },
  },
} as const;
export const KAESO_SLIDER_DEFAULT_VARIANTS = { size: "md" } as const;

export interface SliderProps {
  value?: number | number[] | undefined;
  defaultValue?: number | number[] | undefined;
  onValueChange?: ((value: number | number[]) => void) | undefined;
  onValueCommitted?: ((value: number | number[]) => void) | undefined;
  min?: number | undefined;
  max?: number | undefined;
  step?: number | undefined;
  ticks?: boolean | number[] | undefined;
  formatValue?: ((value: number) => string) | undefined;
  disabled?: boolean | undefined;
  name?: string | undefined;
  "aria-label"?: string | undefined;
  getAriaLabel?: ((index: number) => string) | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const Slider = forwardRef<HTMLDivElement, SliderProps>(function Slider(
  {
    value,
    defaultValue,
    onValueChange,
    onValueCommitted,
    min = 0,
    max = 100,
    step = 1,
    ticks,
    formatValue: formatValueProp,
    disabled,
    name,
    className,
    "aria-label": ariaLabel,
    getAriaLabel,
    "data-force": force,
  },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const formatValue =
    formatValueProp ?? (locale === "en" ? String : (v: number) => new Intl.NumberFormat(locale, { useGrouping: false, maximumFractionDigits: 20 }).format(v));
  const initial = value ?? defaultValue ?? min;
  const count = Array.isArray(initial) ? initial.length : 1;
  const tickValues = Array.isArray(ticks) ? ticks : ticks ? Array.from({ length: Math.floor((max - min) / step) + 1 }, (_, i) => min + i * step) : [];
  return (
    <BaseSlider.Root
      ref={ref}
      {...(value !== undefined ? { value } : {})}
      {...(defaultValue !== undefined ? { defaultValue } : {})}
      {...(onValueChange ? { onValueChange: (v: number | readonly number[]) => onValueChange(Array.isArray(v) ? [...v] : (v as number)) } : {})}
      {...(onValueCommitted ? { onValueCommitted: (v: number | readonly number[]) => onValueCommitted(Array.isArray(v) ? [...v] : (v as number)) } : {})}
      min={min}
      max={max}
      step={step}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(name ? { name } : {})}
      {...(locale === "en" ? {} : { locale })}
      className={cn("relative w-full min-w-120 touch-none select-none data-disabled:opacity-40", className)}
      {...(force ? { "data-force": force } : {})}
    >
      <BaseSlider.Control
        className={cn(
          "relative flex w-full cursor-pointer items-center px-8 data-disabled:cursor-not-allowed coarse:h-44",
          KAESO_SLIDER_VARIANTS.size.md.classes,
        )}
      >
        <BaseSlider.Track className="relative h-4 w-full rounded-pill slider-track">
          <BaseSlider.Indicator className="rounded-pill slider-range" />
          {Array.from({ length: count }, (_, i) => (
            <BaseSlider.Thumb
              key={i}
              index={i}
              {...(getAriaLabel ? { getAriaLabel } : ariaLabel ? { "aria-label": ariaLabel } : {})}
              {...(count === 2
                ? {
                    getAriaValueText: (formatted: string, _v: number, index: number) =>
                      index === 0
                        ? t("slider.rangeStart", "Start of range: {value}", { value: formatted })
                        : t("slider.rangeEnd", "End of range: {value}", { value: formatted }),
                  }
                : {})}
              {...(force ? { "data-force": force } : {})}
              className="group/thumb size-16 rounded-pill slider-thumb"
            >
              <span
                aria-hidden
                className={cn(
                  "pointer-events-none absolute bottom-full left-1/2 mb-8 inline-flex h-24 -translate-x-1/2 items-center rounded-pill bg-tooltip px-8 text-caption font-medium tabular text-fg opacity-0 shadow-xs transition-opacity duration-120 group-data-dragging/thumb:opacity-100 group-focus-within-visible/thumb:opacity-100",
                  (force?.includes("dragging") || force?.includes("focus")) && "opacity-100",
                )}
              >
                <BaseSlider.Value>{(_, values) => formatValue(values[i] ?? 0)}</BaseSlider.Value>
              </span>
            </BaseSlider.Thumb>
          ))}
        </BaseSlider.Track>
      </BaseSlider.Control>
      {tickValues.length > 0 && (
        <div className="pointer-events-none relative -mt-4 h-4 coarse:-mt-10" aria-hidden>
          {tickValues.map((tick) => (
            <span
              key={tick}
              className="absolute top-0 h-4 w-1 -translate-x-1/2 bg-line-strong forced-line"
              style={{ left: `calc(8px + (100% - 16px) * ${(tick - min) / (max - min)})` }}
            />
          ))}
        </div>
      )}
    </BaseSlider.Root>
  );
});
Slider.displayName = "Slider";
