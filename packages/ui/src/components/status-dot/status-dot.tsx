import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { devWarn } from "../../utils/dev.js";
import { Icon, type IconData } from "../icon/icon.js";

export const KAESO_STATUS_DOT_VARIANTS = {
  tone: {
    success: { classes: "text-success-text", description: "Healthy, active, succeeded" },
    warning: { classes: "text-warning-text", description: "Degraded, expiring" },
    danger: { classes: "text-danger-text", description: "Failed, disabled, down" },
    info: { classes: "text-info-text", description: "Informational" },
    neutral: { classes: "text-fg-secondary", description: "Inactive, archived (default)" },
  },
} as const;
export const KAESO_STATUS_DOT_DEFAULT_VARIANTS = { tone: "neutral" } as const;

export interface StatusDotProps {
  tone?: keyof typeof KAESO_STATUS_DOT_VARIANTS.tone | undefined;
  children?: ReactNode | undefined;
  size?: "sm" | "md" | undefined;
  icon?: IconData | undefined;
  className?: string | undefined;
}

export const StatusDot = forwardRef<HTMLSpanElement, StatusDotProps>(function StatusDot({ tone = "neutral", children, size = "md", icon, className }, ref) {
  if (children === undefined || children === null || children === "")
    devWarn("StatusDot: a status is a word in its color; pass the word as children (it renders nothing without one).");
  return (
    <span
      ref={ref}
      className={cn(
        "inline-flex items-center gap-6 font-medium",
        size === "md" ? "text-body" : "text-small",
        KAESO_STATUS_DOT_VARIANTS.tone[tone].classes,
        className,
      )}
    >
      {icon && <Icon icon={icon} size="sm" />}
      {children}
    </span>
  );
});
StatusDot.displayName = "StatusDot";

export const Status = StatusDot;
