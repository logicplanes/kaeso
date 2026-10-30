import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";

export const KAESO_FILTER_TOGGLE_VARIANTS = {
  size: {
    sm: { classes: "h-32 px-12 gap-6 text-body", description: "32px: beside small selects in toolbars and filter rows (default)" },
    md: { classes: "h-36 px-16 gap-8 text-body", description: "36px: beside default selects" },
  },
} as const;
export const KAESO_FILTER_TOGGLE_DEFAULT_VARIANTS = { size: "sm" } as const;

export interface FilterToggleProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "onChange" | "type"> {
  label: ReactNode;
  pressed: boolean;
  onPressedChange: (pressed: boolean) => void;
  size?: keyof typeof KAESO_FILTER_TOGGLE_VARIANTS.size | undefined;
}

export const FilterToggle = forwardRef<HTMLButtonElement, FilterToggleProps>(function FilterToggle(
  { label, pressed, onPressedChange, size = "sm", className, onClick, ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      aria-pressed={pressed}
      onClick={(e) => {
        onClick?.(e);
        if (!e.defaultPrevented) onPressedChange(!pressed);
      }}
      className={cn(
        "inline-flex min-w-0 shrink-0 cursor-pointer items-center rounded-pill font-medium whitespace-nowrap material-selector transition-control focus-ring",
        "disabled:cursor-not-allowed disabled:text-fg-disabled",
        pressed ? "text-fg" : "font-regular text-fg-secondary hover:text-fg",
        KAESO_FILTER_TOGGLE_VARIANTS.size[size].classes,
        className,
      )}
      {...props}
    >
      {pressed && <Icon icon={icons.check} size="sm" className="-ml-2 text-fg" />}
      <span className="truncate">{label}</span>
    </button>
  );
});
FilterToggle.displayName = "FilterToggle";
