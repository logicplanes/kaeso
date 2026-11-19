import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useGutterPadded, useInset } from "../../utils/inset.js";

export const KAESO_EMPTY_VARIANTS = {
  size: {
    lg: { classes: "items-start py-80 gap-24 text-start", description: "A page with nothing in it: a large title, hung from the page's edge (default)" },
    md: { classes: "items-start py-48 gap-16 text-start", description: "Inside a section: hung from the section's edge" },
    compact: { classes: "items-center py-32 gap-8 text-center", description: "Inside tables and lists: one line, centered in the row" },
  },
} as const;
export const KAESO_EMPTY_DEFAULT_VARIANTS = { size: "lg" } as const;

const TITLE = { lg: "text-title-lg", md: "text-title", compact: "text-body" } as const;
const TEXT = { lg: "gap-8", md: "gap-6", compact: "items-center gap-2" } as const;
const DESCRIPTION = { lg: "text-body-lg", md: "text-body", compact: "text-small" } as const;

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
  const inset = useInset();
  const padded = useGutterPadded();
  return (
    <div ref={ref} className={cn("flex flex-col", KAESO_EMPTY_VARIANTS.size[size].classes, inset ? "px-20" : !padded && "px-24", className)}>
      <div className={cn("flex flex-col", TEXT[size])}>
        <h2 className={cn("font-semibold text-fg text-balance", TITLE[size])}>{title}</h2>
        {description && <p className={cn("max-w-44ch text-fg-secondary text-pretty", DESCRIPTION[size])}>{description}</p>}
      </div>
      {actions && <div className={cn("flex flex-wrap items-center gap-8", compact && "justify-center")}>{actions}</div>}
      {children}
    </div>
  );
});
Empty.displayName = "Empty";
