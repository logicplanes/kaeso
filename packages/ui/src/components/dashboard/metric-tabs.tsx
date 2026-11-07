import { forwardRef, useRef, type KeyboardEvent, type ReactNode } from "react";
import { useIndicator } from "../../hooks/use-indicator.js";
import { cn } from "../../utils/cn.js";
import { Sparkline } from "../charts/sparkline.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { Delta } from "../stat/delta.js";
import type { Change } from "./dashboard.js";
import { SwapValue, textOf } from "./swap-value.js";

export interface MetricTabItem {
  id: string;
  label: ReactNode;
  value: ReactNode;
  change?: Change | undefined;
  detail?: ReactNode | undefined;
  trend?: number[] | undefined;
  trendColor?: string | undefined;
  trendLabel?: string | undefined;
}

const TAB_COLS: Record<number, string> = { 2: "lg:grid-cols-2", 3: "lg:grid-cols-3", 4: "lg:grid-cols-4", 5: "lg:grid-cols-5", 6: "lg:grid-cols-6" };

export const MetricTabs = forwardRef<
  HTMLDivElement,
  {
    items: MetricTabItem[];
    value: string;
    onValueChange: (id: string) => void;
    loading?: boolean | undefined;
    "aria-label": string;
    controls?: string | undefined;
    orientation?: "horizontal" | "vertical" | undefined;
    className?: string | undefined;
  }
>(function MetricTabs({ items, value, onValueChange, loading, "aria-label": ariaLabel, controls, orientation = "horizontal", className }, ref) {
  const vertical = orientation === "vertical";
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKey = (e: KeyboardEvent, i: number) => {
    const next = vertical ? "ArrowDown" : "ArrowRight";
    const prev = vertical ? "ArrowUp" : "ArrowLeft";
    const step = e.key === next ? 1 : e.key === prev ? -1 : 0;
    const to = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : step ? (i + step + items.length) % items.length : -1;
    if (to < 0) return;
    e.preventDefault();
    tabs.current[to]?.focus();
    onValueChange(items[to]!.id);
  };
  const odd = items.length % 2 === 1;
  const root = useRef<HTMLDivElement | null>(null);
  const ind = useIndicator(root, '[aria-selected="true"]', `${value}|${items.length}`);
  return (
    <div
      ref={(el) => {
        root.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      role="tablist"
      aria-label={ariaLabel}
      aria-orientation={orientation}
      className={cn(
        "relative gap-1 bg-line",
        !vertical && "grid",
        vertical ? "grid grid-cols-2 xl:flex xl:flex-col" : cn("col-span-full grid-cols-2", TAB_COLS[items.length]),
        className,
      )}
    >
      {items.map((item, i) => {
        const selected = item.id === value;
        return (
          <button
            key={item.id}
            ref={(el) => {
              tabs.current[i] = el;
            }}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={controls}
            tabIndex={selected ? 0 : -1}
            aria-busy={loading || undefined}
            onClick={() => onValueChange(item.id)}
            onKeyDown={(e) => onKey(e, i)}
            className={cn(
              "relative flex min-w-0 cursor-pointer flex-col bg-raised text-left transition-control focus-ring-inset",
              vertical ? "items-stretch justify-center px-20 pt-14 pb-16" : "items-start px-20 pt-16 pb-16",
              !selected && "hover:bg-raised-hover",
              vertical && "flex-1",
              !vertical && odd && i === items.length - 1 && "col-span-2 lg:col-span-1",
            )}
          >
            {vertical ? (
              <>
                <span className="flex h-18 w-full min-w-0 items-center justify-between gap-8">
                  <span className={cn("truncate text-small font-medium transition-control", selected ? "text-fg" : "text-fg-secondary")}>{item.label}</span>
                  {!loading && item.change && <Delta value={item.change.value} better={item.change.better ?? "up"} />}
                </span>
                <span className="mt-4 flex h-28 w-full min-w-0 items-center">
                  {loading ? (
                    <Skeleton shape="line" className="w-80" />
                  ) : (
                    <SwapValue text={textOf(item.value)} className="truncate text-title font-semibold text-fg tabular">
                      {item.value}
                    </SwapValue>
                  )}
                </span>
                <span className="mt-2 h-16 max-w-full truncate text-caption text-fg-tertiary">{!loading && item.detail}</span>
                {item.trend && (
                  <span className="mt-8 block h-28 w-full">
                    {!loading && item.trend.length > 1 && (
                      <Sparkline
                        values={item.trend}
                        fluid
                        height={28}
                        aria-label={item.trendLabel ?? ""}
                        {...(item.trendColor ? { color: item.trendColor } : {})}
                      />
                    )}
                  </span>
                )}
              </>
            ) : (
              <>
                <span className={cn("max-w-full truncate text-small font-medium transition-control", selected ? "text-fg" : "text-fg-secondary")}>
                  {item.label}
                </span>
                <span className="mt-6 flex h-34 w-full min-w-0 items-center">
                  {loading ? (
                    <Skeleton shape="line" className="h-20 w-96" />
                  ) : (
                    <SwapValue text={textOf(item.value)} className="truncate text-metric font-semibold text-fg tabular">
                      {item.value}
                    </SwapValue>
                  )}
                </span>
                <span className="mt-2 flex h-20 w-full min-w-0 items-center gap-6">
                  {loading ? (
                    <Skeleton shape="line" className="w-80" />
                  ) : (
                    <>
                      {item.change && <Delta value={item.change.value} better={item.change.better ?? "up"} />}
                      {item.detail && <span className="truncate text-small text-fg-tertiary">{item.detail}</span>}
                    </>
                  )}
                </span>
                {item.trend && (
                  <span className="mt-8 block h-40 w-full">
                    {!loading && item.trend.length > 1 && (
                      <Sparkline values={item.trend} fluid aria-label={item.trendLabel ?? ""} {...(item.trendColor ? { color: item.trendColor } : {})} />
                    )}
                  </span>
                )}
              </>
            )}
          </button>
        );
      })}
      <span
        aria-hidden
        className={cn("pointer-events-none absolute top-0 left-0 bg-fg", ind.animate && "indicator-motion", !ind.ready && "opacity-0")}
        style={
          vertical
            ? { transform: `translate(${ind.x}px, ${ind.y}px)`, width: 2, height: ind.h }
            : { transform: `translate(${ind.x}px, ${ind.y + ind.h - 2}px)`, width: ind.w, height: 2 }
        }
      />
    </div>
  );
});
MetricTabs.displayName = "MetricTabs";
