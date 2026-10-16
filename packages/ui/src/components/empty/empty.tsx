import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_EMPTY_VARIANTS = {
  size: {
    lg: { classes: "py-80 gap-16", description: "Centered in a panel (default)" },
    md: { classes: "py-48 gap-12", description: "Inside a section" },
    compact: { classes: "py-32 gap-8", description: "Inside tables and lists" },
  },
} as const;
export const KAESO_EMPTY_DEFAULT_VARIANTS = { size: "lg" } as const;

export interface EmptyProps {
  title: ReactNode;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  size?: keyof typeof KAESO_EMPTY_VARIANTS.size | undefined;
  children?: ReactNode | undefined;
  className?: string | undefined;
}

export const Empty = forwardRef<HTMLDivElement, EmptyProps>(function Empty({ title, description, actions, size = "lg", children, className }, ref) {
  const compact = size === "compact";
  return (
    <div ref={ref} className={cn("flex flex-col items-center text-center", KAESO_EMPTY_VARIANTS.size[size].classes, className)}>
      <div className={cn("flex flex-col items-center", compact ? "gap-2" : "gap-6")}>
        <h2 className={cn("font-semibold text-fg text-balance", compact ? "text-body" : "text-title")}>{title}</h2>
        {description && <p className={cn("max-w-44ch text-fg-secondary text-pretty", compact ? "text-small" : "text-body")}>{description}</p>}
      </div>
      {actions && <div className={cn("flex flex-wrap items-center justify-center gap-8", !compact && "mt-8")}>{actions}</div>}
      {children}
    </div>
  );
});
Empty.displayName = "Empty";
