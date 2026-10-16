import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_SEPARATOR_VARIANTS = {
  orientation: {
    horizontal: { classes: "h-1 w-full", description: "A 1px line across" },
    vertical: { classes: "w-1 self-stretch", description: "A 1px line down" },
  },
  tone: {
    subtle: { classes: "bg-line-subtle", description: "Inside groups and tables (default)" },
    default: { classes: "bg-line", description: "Between regions that tone and space cannot separate" },
  },
} as const;
export const KAESO_SEPARATOR_DEFAULT_VARIANTS = { orientation: "horizontal", tone: "subtle" } as const;

export interface SeparatorProps extends HTMLAttributes<HTMLDivElement> {
  orientation?: keyof typeof KAESO_SEPARATOR_VARIANTS.orientation | undefined;
  tone?: keyof typeof KAESO_SEPARATOR_VARIANTS.tone | undefined;
  label?: ReactNode | undefined;
}

export const Separator = forwardRef<HTMLDivElement, SeparatorProps>(function Separator(
  { orientation = "horizontal", tone = "subtle", label, className, ...props },
  ref,
) {
  if (label && orientation === "horizontal") {
    return (
      <div ref={ref} role="separator" aria-orientation="horizontal" className={cn("flex w-full items-center gap-12", className)} {...props}>
        <span className={cn("h-1 flex-1", KAESO_SEPARATOR_VARIANTS.tone[tone].classes)} />
        <span className="text-caption font-medium text-fg-tertiary">{label}</span>
        <span className={cn("h-1 flex-1", KAESO_SEPARATOR_VARIANTS.tone[tone].classes)} />
      </div>
    );
  }
  return (
    <div
      ref={ref}
      role="separator"
      aria-orientation={orientation}
      className={cn("shrink-0", KAESO_SEPARATOR_VARIANTS.orientation[orientation].classes, KAESO_SEPARATOR_VARIANTS.tone[tone].classes, className)}
      {...props}
    />
  );
});
Separator.displayName = "Separator";
