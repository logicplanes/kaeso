import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { TruncatedText } from "../text/truncated.js";

export const KAESO_BADGE_VARIANTS = {
  tone: {
    neutral: { classes: "bg-fill-2 mark-fill text-fg-secondary", description: "Neutral (default): fill-2, or the control fill on a hovered or selected row" },
    success: { classes: "bg-success-tint text-success-text", description: "Success" },
    warning: { classes: "bg-warning-tint text-warning-text", description: "Warning" },
    danger: { classes: "bg-danger-tint text-danger-text", description: "Danger" },
    info: { classes: "bg-info-tint text-info-text", description: "Information" },
    outline: { classes: "bg-transparent text-fg-secondary shadow-ring", description: "A ring only" },
    ink: { classes: "bg-ink text-on-ink", description: "Inverted: at most one on a view, for the one thing that must stand out" },
  },
} as const;
export const KAESO_BADGE_DEFAULT_VARIANTS = { tone: "neutral" } as const;

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: keyof typeof KAESO_BADGE_VARIANTS.tone | undefined;
  children: ReactNode;
}

export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(function Badge({ tone = "neutral", className, children, ...props }, ref) {
  return (
    <TruncatedText
      ref={ref}
      className={cn(
        "inline-block h-22 max-w-full min-w-0 shrink-0 rounded-pill px-8 py-3 align-middle text-caption font-medium tabular",
        "forced-colors:outline forced-colors:-outline-offset-1",
        KAESO_BADGE_VARIANTS.tone[tone].classes,
        className,
      )}
      {...props}
    >
      {children}
    </TruncatedText>
  );
});
Badge.displayName = "Badge";
