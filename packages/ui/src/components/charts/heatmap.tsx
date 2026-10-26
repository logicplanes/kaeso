import { forwardRef, useLayoutEffect, useRef, useState } from "react";
import { max as d3max } from "d3-array";
import { cn } from "../../utils/cn.js";
import { formatNumber } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import { useScrollFadeX } from "../../hooks/use-scroll-fade.js";
import { ChartFrame, ChartLive, ChartTooltip } from "./core.js";

export const KAESO_HEATMAP_VARIANTS = {
  layout: {
    week: { classes: "", description: "Hour × weekday (default)" },
    year: { classes: "", description: "Day × week" },
  },
} as const;
export const KAESO_HEATMAP_DEFAULT_VARIANTS = { layout: "week" } as const;

const RAMP = [
  "var(--k-fill-1)",
  "color-mix(in oklab, var(--k-viz-1) 40%, var(--k-fill-1))",
  "color-mix(in oklab, var(--k-viz-1) 60%, var(--k-fill-1))",
  "color-mix(in oklab, var(--k-viz-1) 80%, var(--k-fill-1))",
  "var(--k-viz-1)",
];
const CELL = 12;
const GAP = 3;
const HEAD = 16;

export const Heatmap = forwardRef<
  HTMLDivElement,
  {
    rows: string[];
    columns: string[];
    values: number[][];
    formatValue?: ((v: number) => string) | undefined;
    "aria-label": string;
    className?: string | undefined;
    columnLabelEvery?: number | undefined;
  }
>(function Heatmap({ rows, columns, values, formatValue = (v) => formatNumber(v), className, "aria-label": ariaLabel, columnLabelEvery = 3 }, ref) {
  const t = useUiText();
  const hi = d3max(values.flat().filter(Number.isFinite)) ?? 0;
  const level = (v: number) => (!Number.isFinite(v) || v <= 0 || hi === 0 ? 0 : Math.min(4, Math.ceil((v / hi) * 4)));
  const empty = rows.length === 0 || columns.length === 0;
  const [cell, setCell] = useState<{ r: number; c: number } | null>(null);
  const [keyboard, setKeyboard] = useState(false);
  const scroller = useRef<HTMLDivElement | null>(null);
  const grid = useRef<HTMLDivElement | null>(null);
  const [labelWidth, setLabelWidth] = useState(0);
  useScrollFadeX(scroller, "[data-heatmap-grid]");
  useLayoutEffect(() => {
    const first = grid.current?.querySelector<HTMLElement>("[data-heatmap-rows]");
    if (first) setLabelWidth(first.offsetWidth);
  }, [rows]);
  useLayoutEffect(() => {
    if (!keyboard || !cell) return;
    grid.current?.querySelector(`[data-cell="${cell.r}-${cell.c}"]`)?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [cell, keyboard]);
  const value = (r: number, c: number) => values[r]?.[c] ?? 0;
  const line = (r: number, c: number) =>
    t("heatmap.cell", "{row} {column}: {value}", { row: rows[r] ?? "", column: columns[c] ?? "", value: formatValue(value(r, c)) });
  const height = HEAD + GAP + rows.length * (CELL + GAP) + 8;
  const table = { columns: ["", ...columns], rows: rows.map((r, ri) => [r, ...columns.map((_, ci) => formatValue(value(ri, ci)))]) };
  const move = (key: string) => {
    const at = cell ?? { r: 0, c: 0 };
    const next =
      key === "ArrowRight"
        ? { r: at.r, c: Math.min(columns.length - 1, at.c + 1) }
        : key === "ArrowLeft"
          ? { r: at.r, c: Math.max(0, at.c - 1) }
          : key === "ArrowDown"
            ? { r: Math.min(rows.length - 1, at.r + 1), c: at.c }
            : key === "ArrowUp"
              ? { r: Math.max(0, at.r - 1), c: at.c }
              : key === "Home"
                ? { r: at.r, c: 0 }
                : key === "End"
                  ? { r: at.r, c: columns.length - 1 }
                  : undefined;
    return cell === null && next ? at : next;
  };
  const scrollLeft = scroller.current?.scrollLeft ?? 0;
  const tip = cell
    ? {
        x: labelWidth + GAP + cell.c * (CELL + GAP) + CELL - scrollLeft,
        xLeft: labelWidth + GAP + cell.c * (CELL + GAP) - scrollLeft,
        y: HEAD + GAP + cell.r * (CELL + GAP) + 24,
      }
    : null;
  return (
    <div ref={ref} className={cn("w-full min-w-0", className)}>
      <ChartFrame
        label={ariaLabel}
        height={height}
        table={empty ? undefined : table}
        state={{ empty }}
        legend={
          empty ? undefined : (
            <div className="flex items-center gap-3 text-caption font-medium text-fg-tertiary" aria-hidden>
              <span className="mr-4">{t("heatmap.less", "Less")}</span>
              {RAMP.map((c) => (
                <span key={c} className="size-12 rounded-cell data-mark" style={{ background: c }} />
              ))}
              <span className="ml-4">{t("heatmap.more", "More")}</span>
            </div>
          )
        }
      >
        {!empty && (
          <div className="absolute inset-0">
            <div
              ref={scroller}
              role="group"
              tabIndex={0}
              aria-label={ariaLabel}
              onKeyDown={(e) => {
                if (e.key === "Escape" && cell) {
                  e.stopPropagation();
                  setCell(null);
                  return;
                }
                const next = move(e.key);
                if (!next) return;
                e.preventDefault();
                setKeyboard(true);
                setCell(next);
              }}
              onFocus={(e) => {
                if (e.target !== e.currentTarget || document.documentElement.hasAttribute("data-k-pointer")) return;
                let visible = true;
                try {
                  visible = e.currentTarget.matches(":focus-visible");
                } catch {}
                if (visible) {
                  setKeyboard(true);
                  setCell({ r: 0, c: 0 });
                }
              }}
              onBlur={() => {
                setCell(null);
                setKeyboard(false);
              }}
              onScroll={() => setCell((c) => (keyboard ? c : null))}
              className="size-full overflow-x-auto overflow-y-hidden rounded-cell scrollbar-quiet scroll-fade-x focus-ring-inset"
            >
              <div
                ref={grid}
                data-heatmap-grid=""
                className="inline-grid gap-3"
                style={{ gridTemplateColumns: `max-content repeat(${columns.length}, ${CELL}px)` }}
                onPointerLeave={() => {
                  setKeyboard(false);
                  setCell(null);
                }}
              >
                <span data-heatmap-rows="" />
                {columns.map((c, i) => (
                  <span key={c} className="h-16 overflow-visible text-caption whitespace-nowrap text-fg-tertiary tabular" aria-hidden>
                    {i % columnLabelEvery === 0 ? c : ""}
                  </span>
                ))}
                {rows.map((r, ri) => (
                  <div key={r} className="contents">
                    <span className="flex h-12 items-center pr-8 text-caption whitespace-nowrap text-fg-tertiary" aria-hidden>
                      {r}
                    </span>
                    {columns.map((c, ci) => {
                      const on = cell?.r === ri && cell.c === ci;
                      return (
                        <span
                          key={c}
                          data-cell={`${ri}-${ci}`}
                          aria-hidden
                          onPointerEnter={() => {
                            setKeyboard(false);
                            setCell({ r: ri, c: ci });
                          }}
                          className={cn("size-12 rounded-cell data-mark", on && "outline-2 outline-offset-1 outline-fg")}
                          style={{ background: RAMP[level(value(ri, ci))] }}
                        />
                      );
                    })}
                  </div>
                ))}
              </div>
            </div>
            {cell && tip && (
              <ChartTooltip
                x={tip.x}
                xLeft={tip.xLeft}
                y={tip.y}
                width={scroller.current?.clientWidth ?? 0}
                visible
                heading={t("heatmap.cellHeading", "{row} {column}", { row: rows[cell.r] ?? "", column: columns[cell.c] ?? "" })}
                rows={[{ id: "v", label: "", color: RAMP[level(value(cell.r, cell.c))]!, value: formatValue(value(cell.r, cell.c)) }]}
              />
            )}
            <ChartLive>{keyboard && cell ? line(cell.r, cell.c) : null}</ChartLive>
          </div>
        )}
      </ChartFrame>
    </div>
  );
});
Heatmap.displayName = "Heatmap";
