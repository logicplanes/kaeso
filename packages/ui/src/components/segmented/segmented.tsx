import { forwardRef, useRef, type ReactNode } from "react";
import { RadioGroup as BaseRadioGroup } from "@base-ui/react/radio-group";
import { Radio as BaseRadio } from "@base-ui/react/radio";
import { cn } from "../../utils/cn.js";
import { useControllable } from "../../utils/use-controllable.js";
import { useIndicator } from "../../hooks/use-indicator.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Tooltip } from "../tooltip/tooltip.js";

export const KAESO_SEGMENTED_VARIANTS = {
  size: {
    sm: { classes: "h-32 p-2 gap-2", description: "32px track, 28px items: toolbars and popovers" },
    md: { classes: "h-36 p-2 gap-2", description: "36px track, 32px items (default)" },
  },
} as const;
export const KAESO_SEGMENTED_DEFAULT_VARIANTS = { size: "md" } as const;

const ITEM = { sm: "h-28 px-10 text-body", md: "h-32 px-12 text-body" } as const;
const ITEM_ICON = { sm: "w-32 px-0", md: "w-36 px-0" } as const;

export interface SegmentedOption {
  value: string;
  label?: ReactNode | undefined;
  icon?: IconData | undefined;
  "aria-label"?: string | undefined;
  disabled?: boolean | undefined;
}

export interface SegmentedProps {
  options: SegmentedOption[];
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  size?: keyof typeof KAESO_SEGMENTED_VARIANTS.size | undefined;
  disabled?: boolean | undefined;
  fullWidth?: boolean | undefined;
  "aria-label": string;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const Segmented = forwardRef<HTMLDivElement, SegmentedProps>(function Segmented(
  { options, value, defaultValue, onValueChange, size = "md", disabled, fullWidth, className, "aria-label": ariaLabel, "data-force": force },
  ref,
) {
  const [current, setCurrent] = useControllable(value, defaultValue ?? options[0]?.value ?? "", onValueChange);
  const track = useRef<HTMLDivElement | null>(null);
  const ind = useIndicator(track, "[data-checked]", `${current}|${options.length}|${size}`);
  return (
    <BaseRadioGroup
      ref={(el: HTMLDivElement | null) => {
        track.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      value={current}
      onValueChange={(v: unknown) => setCurrent(String(v))}
      {...(disabled !== undefined ? { disabled } : {})}
      aria-label={ariaLabel}
      {...(force ? { "data-force": force } : {})}
      className={cn(
        "relative isolate inline-flex items-center rounded-pill bg-fill-1",
        KAESO_SEGMENTED_VARIANTS.size[size].classes,
        fullWidth && "flex w-full",
        disabled && "opacity-40",
        className,
      )}
    >
      <span
        aria-hidden
        className={cn("absolute top-0 left-0 -z-10 rounded-pill bg-raised shadow-segment", ind.animate && "indicator-motion", !ind.ready && "opacity-0")}
        style={{ transform: `translate(${ind.x}px, ${ind.y}px)`, width: ind.w, height: ind.h }}
      />
      {options.map((o) => {
        const iconOnly = o.icon && !o.label;
        const item = (
          <BaseRadio.Root
            key={o.value}
            value={o.value}
            {...(o.disabled ? { disabled: true } : {})}
            {...(iconOnly && o["aria-label"] ? { "aria-label": o["aria-label"] } : {})}
            className={cn(
              "relative inline-flex cursor-pointer items-center justify-center gap-6 rounded-pill font-medium whitespace-nowrap text-fg-secondary transition-control focus-ring tap-target",
              "hover:text-fg checked:text-fg disabled:cursor-not-allowed disabled:text-fg-disabled",
              ITEM[size],
              iconOnly && ITEM_ICON[size],
              fullWidth && "flex-1",
            )}
          >
            {o.icon && <Icon icon={o.icon} size="sm" />}
            {o.label}
          </BaseRadio.Root>
        );
        return iconOnly && o["aria-label"] ? (
          <Tooltip key={o.value} content={o["aria-label"]}>
            {item}
          </Tooltip>
        ) : (
          item
        );
      })}
    </BaseRadioGroup>
  );
});
Segmented.displayName = "Segmented";
