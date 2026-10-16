import { forwardRef } from "react";
import { Slider as BaseSlider } from "@base-ui/react/slider";
import { cn } from "../../utils/cn";
import { useLocale, useUiText } from "../../i18n/context";

export const KAESO_SLIDER_VARIANTS = {
  size: {
    md: { classes: "h-20", description: "A 4px track in a 20px hit area (default)" },
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
      <BaseSlider.Control className="relative flex h-20 w-full cursor-pointer items-center data-disabled:cursor-not-allowed">
        <BaseSlider.Track className="relative h-4 w-full rounded-pill bg-fill-3">
          <BaseSlider.Indicator className="rounded-pill bg-ink" />
          {Array.from({ length: count }, (_, i) => (
            <BaseSlider.Thumb
              key={i}
              index={i}
              {...(getAriaLabel ? { getAriaLabel } : ariaLabel ? { "aria-label": ariaLabel } : {})}
              {...(count === 2
                ? {
                    getAriaValueText: (formatted: string, _v: number, index: number) =>
                      index === 0
                        ? t("slider.rangeStart", "{value} start range", { value: formatted })
                        : t("slider.rangeEnd", "{value} end range", { value: formatted }),
                  }
                : {})}
              className={cn(
                "group/thumb size-16 rounded-pill bg-ink ring-thumb outline-2 outline-offset-2 outline-transparent transition-control focus-within-visible:outline-focus",
                force?.includes("focus") && "outline-focus",
              )}
            >
              <span
                aria-hidden
                className={cn(
                  "pointer-events-none absolute bottom-full left-1/2 mb-8 inline-flex h-24 -translate-x-1/2 items-center rounded-pill bg-tooltip px-8 text-caption font-medium tabular text-fg opacity-0 shadow-raised transition-opacity duration-120 group-data-dragging/thumb:opacity-100",
                  force?.includes("dragging") && "opacity-100",
                )}
              >
                <BaseSlider.Value>{(_, values) => formatValue(values[i] ?? 0)}</BaseSlider.Value>
              </span>
            </BaseSlider.Thumb>
          ))}
        </BaseSlider.Track>
      </BaseSlider.Control>
      {tickValues.length > 0 && (
        <div className="relative mt-2 h-4" aria-hidden>
          {tickValues.map((tick) => (
            <span key={tick} className="absolute top-0 h-4 w-1 -translate-x-1/2 bg-line-strong" style={{ left: `${((tick - min) / (max - min)) * 100}%` }} />
          ))}
        </div>
      )}
    </BaseSlider.Root>
  );
});
Slider.displayName = "Slider";
