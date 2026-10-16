import { forwardRef, type HTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useInset } from "../../utils/inset";
import { arrowFocus } from "../../utils/arrow-focus";
import { useUiText } from "../../i18n/context";

export const KAESO_FILTER_ROW_VARIANTS = {
  size: {
    md: { classes: "px-24 py-8", description: "Under a page header on an edge-to-edge page: 24px sides, 48px tall on one line like a toolbar" },
    inset: {
      classes: "pt-16 pb-16",
      description: "In a settings column: 24px under the header (with the column's 8px), 16px above the list or table (chosen automatically)",
    },
  },
} as const;
export const KAESO_FILTER_ROW_DEFAULT_VARIANTS = { size: "md" } as const;

export interface FilterRowProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  tabs?: ReactNode | undefined;
  children?: ReactNode | undefined;
  filtersLabel?: string | undefined;
  size?: keyof typeof KAESO_FILTER_ROW_VARIANTS.size | undefined;
}

export const FilterRow = forwardRef<HTMLDivElement, FilterRowProps>(function FilterRow(
  { tabs, children, filtersLabel, size: sizeProp, className, ...props },
  ref,
) {
  const inset = useInset();
  const t = useUiText();
  const size = sizeProp ?? (inset ? "inset" : "md");
  return (
    <div
      ref={ref}
      className={cn("flex min-w-0 shrink-0 flex-wrap items-center justify-between gap-x-16 gap-y-8", KAESO_FILTER_ROW_VARIANTS.size[size].classes, className)}
      {...props}
    >
      {tabs && <div className="flex max-w-full min-w-0">{tabs}</div>}
      {children && (
        <div
          role="toolbar"
          aria-label={filtersLabel ?? t("filterRow.filters", "Filters")}
          aria-orientation="horizontal"
          className="flex max-w-full min-w-0 flex-wrap items-center gap-8"
          onKeyDown={(e: KeyboardEvent<HTMLDivElement>) => arrowFocus(e)}
        >
          {children}
        </div>
      )}
    </div>
  );
});
FilterRow.displayName = "FilterRow";
