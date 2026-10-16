import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_BADGE_VARIANTS = {
  tone: {
    neutral: { classes: "bg-fill-2 text-fg-secondary", description: "Neutral (default)" },
    success: { classes: "bg-success-tint text-success-text", description: "Success" },
    warning: { classes: "bg-warning-tint text-warning-text", description: "Warning" },
    danger: { classes: "bg-danger-tint text-danger-text", description: "Danger" },
    info: { classes: "bg-info-tint text-info-text", description: "Information" },
    outline: { classes: "bg-transparent text-fg-secondary shadow-ring", description: "A ring only" },
    ink: { classes: "bg-ink text-on-ink", description: "Inverted, for the one thing that must stand out" },
  },
} as const;
export const KAESO_BADGE_DEFAULT_VARIANTS = { tone: "neutral" } as const;

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: keyof typeof KAESO_BADGE_VARIANTS.tone | undefined;
  children: ReactNode;
}

export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(function Badge({ tone = "neutral", className, children, ...props }, ref) {
  return (
    <span
      ref={ref}
      className={cn(
        "inline-flex h-22 shrink-0 items-center rounded-pill px-8 text-caption font-medium whitespace-nowrap tabular",
        KAESO_BADGE_VARIANTS.tone[tone].classes,
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
});
Badge.displayName = "Badge";
