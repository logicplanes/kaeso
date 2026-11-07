import { forwardRef, type ReactNode } from "react";
import { useLinkComponent } from "../../utils/link-provider.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { cn } from "../../utils/cn.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { Delta } from "../stat/delta.js";
import { SwapValue, textOf } from "./swap-value.js";

export const KAESO_DASHBOARD_VARIANTS = {
  surface: {
    grid: {
      classes: "grid gap-1 overflow-hidden rounded-group bg-line shadow-xs forced-line",
      description: "Cells on one surface, split by hairlines like a StatGroup (default)",
    },
  },
} as const;
export const KAESO_DASHBOARD_DEFAULT_VARIANTS = { surface: "grid" } as const;

export interface Change {
  value: number;
  better?: "up" | "down" | undefined;
}

const XL_COLS: Record<number, string> = { 2: "xl:grid-cols-2", 3: "xl:grid-cols-3", 4: "xl:grid-cols-4", 6: "xl:grid-cols-6", 12: "xl:grid-cols-12" };

const LG_COLS: Record<number, string> = {
  2: "lg:grid-cols-2",
  3: "lg:grid-cols-3",
  4: "lg:grid-cols-4",
  5: "lg:grid-cols-5",
  6: "lg:grid-cols-6",
  12: "lg:grid-cols-12",
};

export const Dashboard = forwardRef<
  HTMLDivElement,
  {
    children: ReactNode;
    columns?: 2 | 3 | 4 | 5 | 6 | 12 | undefined;
    wideColumns?: 2 | 3 | 4 | 6 | 12 | undefined;
    busy?: boolean | undefined;
    "aria-label"?: string | undefined;
    className?: string | undefined;
  }
>(function Dashboard({ children, columns = 12, wideColumns, busy, "aria-label": ariaLabel, className }, ref) {
  return (
    <div
      ref={ref}
      role="group"
      aria-label={ariaLabel}
      aria-busy={busy || undefined}
      className={cn(KAESO_DASHBOARD_VARIANTS.surface.grid.classes, "busy-dim grid-cols-1", LG_COLS[columns], wideColumns && XL_COLS[wideColumns], className)}
    >
      {children}
    </div>
  );
});
Dashboard.displayName = "Dashboard";

export const DashboardCell = forwardRef<
  HTMLElement,
  {
    title?: ReactNode | undefined;
    href?: string | undefined;
    actions?: ReactNode | undefined;
    children?: ReactNode | undefined;
    className?: string | undefined;
  }
>(function DashboardCell({ title, href, actions, children, className }, ref) {
  const Link = useLinkComponent();
  return (
    <section ref={ref} className={cn("flex min-w-0 flex-col bg-raised px-8 pt-16 pb-20", className)}>
      {(title || actions) && (
        <header className={cn("flex h-24 min-w-0 items-center justify-between gap-12", actions ? "pr-0" : "pr-12", href ? "pl-8" : "pl-12")}>
          {title &&
            (href ? (
              <h3 className="min-w-0">
                <Link
                  href={href}
                  className="group inline-flex h-24 max-w-full items-center gap-2 rounded-cell px-4 text-small font-medium text-fg-secondary transition-control hover:text-fg focus-ring"
                >
                  <span className="truncate">{title}</span>
                  <Icon icon={icons.chevronRight} size="xs" className="shrink-0 transition-control group-hover:translate-x-1" />
                </Link>
              </h3>
            ) : (
              <h3 className="truncate text-small font-medium text-fg-secondary">{title}</h3>
            ))}
          {actions && <div className="flex shrink-0 items-center gap-4">{actions}</div>}
        </header>
      )}
      <div className={cn("flex min-w-0 flex-1 flex-col", (title || actions) && "mt-6")}>{children}</div>
    </section>
  );
});
DashboardCell.displayName = "DashboardCell";

export const CellInset = forwardRef<HTMLDivElement, { children: ReactNode; grow?: boolean | undefined }>(function CellInset({ children, grow }, ref) {
  return (
    <div ref={ref} className={cn("min-w-0 px-12", grow && "flex flex-1 flex-col")}>
      {children}
    </div>
  );
});
CellInset.displayName = "CellInset";

export interface ReadoutProps {
  label?: ReactNode | undefined;
  value: ReactNode;
  unit?: ReactNode | undefined;
  change?: Change | undefined;
  detail?: ReactNode | undefined;
  size?: keyof typeof KAESO_READOUT_VARIANTS.size | undefined;
  loading?: boolean | undefined;
}

export const KAESO_READOUT_VARIANTS = {
  size: {
    md: { classes: "text-metric", description: "28/32: the cell's main number (default)" },
    sm: { classes: "text-title", description: "20/28: a number beside another, under a small label" },
  },
} as const;
export const KAESO_READOUT_DEFAULT_VARIANTS = { size: "md" } as const;

export const Readout = forwardRef<HTMLDivElement, ReadoutProps>(function Readout({ label, value, unit, change, detail, size = "md", loading }, ref) {
  const md = size === "md";
  return (
    <div ref={ref} className="flex min-w-0 flex-col px-12">
      {label && <span className="truncate text-small text-fg-tertiary">{label}</span>}
      <span className={cn("flex min-w-0 items-center", md ? "h-34" : "mt-2 h-28")}>
        {loading ? (
          <Skeleton shape="line" className={md ? "h-20 w-96" : "w-64"} />
        ) : (
          <span className="flex min-w-0 items-baseline gap-6">
            <SwapValue text={textOf(value)} className={cn("truncate font-semibold text-fg tabular", KAESO_READOUT_VARIANTS.size[size].classes)}>
              {value}
            </SwapValue>
            {unit && <span className="truncate text-small text-fg-secondary">{unit}</span>}
          </span>
        )}
      </span>
      {(change || detail || md) && (
        <span className="mt-2 flex h-20 min-w-0 items-center gap-6">
          {loading ? (
            <Skeleton shape="line" className="w-120" />
          ) : (
            <>
              {change && <Delta value={change.value} better={change.better ?? "up"} />}
              {detail && <span className="truncate text-small text-fg-tertiary">{detail}</span>}
            </>
          )}
        </span>
      )}
    </div>
  );
});
Readout.displayName = "Readout";
