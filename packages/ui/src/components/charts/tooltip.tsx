import { forwardRef, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { mergeRefs } from "../../utils/merge-refs.js";
import { observeResize } from "../../utils/observe-resize.js";
import { edgeColor } from "./marks.js";

export function useSize<T extends HTMLElement>(): [(el: T | null) => void, { width: number; height: number }] {
  const [el, setEl] = useState<T | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    if (!el) return;
    const set = () => setSize({ width: Math.round(el.clientWidth), height: Math.round(el.clientHeight) });
    set();
    return observeResize([el], set);
  }, [el]);
  return [setEl, size];
}

export interface SwatchProps {
  color: string;
  shape?: "square" | "line" | undefined;
  off?: boolean | undefined;
}

export const Swatch = forwardRef<HTMLSpanElement, SwatchProps>(function Swatch({ color, shape = "square", off }, ref) {
  const edge = edgeColor(color);
  return (
    <span
      ref={ref}
      aria-hidden
      data-off={off ? "" : undefined}
      className={cn("shrink-0 data-mark transition-control", shape === "line" ? "h-2 w-12 rounded-pill" : "size-10 rounded-bar")}
      style={
        off
          ? shape === "line"
            ? { outline: `1px solid ${edge ?? color}`, outlineOffset: 1 }
            : { boxShadow: `inset 0 0 0 1px ${edge ?? color}` }
          : { background: color, ...(edge && shape === "square" ? { boxShadow: `inset 0 0 0 1px ${edge}` } : {}) }
      }
    />
  );
});
Swatch.displayName = "Swatch";

export interface TooltipRow {
  id: string;
  label: ReactNode;
  color: string;
  value: ReactNode;
  share?: ReactNode | undefined;
}

export interface ChartTooltipProps {
  x: number;
  xLeft?: number | undefined;
  y: number;
  width: number;
  heading: ReactNode;
  rows: TooltipRow[];
  visible: boolean;
}

export const ChartTooltip = forwardRef<HTMLDivElement, ChartTooltipProps>(function ChartTooltip({ x, xLeft = x, y, width, heading, rows, visible }, ref) {
  const el = useRef<HTMLDivElement | null>(null);
  const root = useMemo(() => mergeRefs(el, ref), [ref]);
  const [own, setOwn] = useState({ w: 0, h: 0, room: Infinity });
  useLayoutEffect(() => {
    const node = el.current;
    const next = { w: node?.offsetWidth ?? 0, h: node?.offsetHeight ?? 0, room: (node?.offsetParent as HTMLElement | null)?.clientHeight ?? Infinity };
    if (next.w !== own.w || next.h !== own.h || next.room !== own.room) setOwn(next);
  }, [own, heading, rows, visible]);
  const flip = x > width / 2;
  const wanted = flip ? xLeft - 16 - own.w : x + 16;
  const left = own.w ? Math.round(Math.max(0, Math.min(wanted, width - own.w))) : undefined;
  const top = Math.round(Math.max(0, Math.min(y - 24, own.room - own.h)));
  return (
    <div
      ref={root}
      role="presentation"
      aria-hidden
      className={cn(
        "pointer-events-none absolute z-10 w-max min-w-160 max-w-360 rounded-tooltip bg-raised p-10 text-small shadow-raised opacity-motion @chart-legend:min-w-200",
        visible && own.w ? "opacity-100" : "opacity-0",
      )}
      style={{
        ...(left !== undefined ? { left } : flip ? { right: Math.round(width - xLeft + 16) } : { left: Math.round(x + 16) }),
        top,
      }}
    >
      <div className="mb-6 text-caption font-medium text-fg tabular">{heading}</div>
      <div className="flex flex-col gap-4">
        {rows.map((r) => (
          <div key={r.id} className="flex items-center gap-8">
            <Swatch color={r.color} />
            <span className="min-w-0 flex-1 truncate pr-12 text-fg-secondary">{r.label}</span>
            <span className="font-medium text-fg tabular">{r.value}</span>
            {r.share !== undefined && <span className="w-40 text-right text-fg-tertiary tabular">{r.share}</span>}
          </div>
        ))}
      </div>
    </div>
  );
});
ChartTooltip.displayName = "ChartTooltip";
