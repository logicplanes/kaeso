import { forwardRef } from "react";
import { max as d3max } from "d3-array";
import { cn } from "../../utils/cn";
import { formatNumber } from "../../format";
import { useUiText } from "../../i18n/context";

export const KAESO_HEATMAP_VARIANTS = {
  layout: {
    week: { classes: "", description: "Hour × weekday (default)" },
    year: { classes: "", description: "Day × week" },
  },
} as const;
export const KAESO_HEATMAP_DEFAULT_VARIANTS = { layout: "week" } as const;

const RAMP = [
  "var(--k-fill-1)",
  "color-mix(in oklab, var(--k-viz-1) 25%, var(--k-fill-1))",
  "color-mix(in oklab, var(--k-viz-1) 50%, var(--k-fill-1))",
  "color-mix(in oklab, var(--k-viz-1) 75%, var(--k-fill-1))",
  "var(--k-viz-1)",
];

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
  const hi = d3max(values.flat()) ?? 0;
  const level = (v: number) => (v <= 0 || hi === 0 ? 0 : Math.min(4, Math.ceil((v / hi) * 4)));
  return (
    <div ref={ref} className={cn("flex min-w-0 max-w-full flex-col gap-12", className)}>
      <div
        role="table"
        tabIndex={0}
        aria-label={ariaLabel}
        className="inline-grid max-w-full gap-3 overflow-x-auto scrollbar-quiet rounded-cell focus-ring"
        style={{ gridTemplateColumns: `40px repeat(${columns.length}, 12px)` }}
      >
        <span />
        {columns.map((c, i) => (
          <span key={c} className="h-16 text-micro text-fg-tertiary tabular whitespace-nowrap" aria-hidden>
            {i % columnLabelEvery === 0 ? c : ""}
          </span>
        ))}
        {rows.map((r, ri) => (
          <div key={r} role="row" className="contents">
            <span role="rowheader" className="flex h-12 items-center text-micro text-fg-tertiary">
              {r}
            </span>
            {columns.map((c, ci) => {
              const v = values[ri]?.[ci] ?? 0;
              const cell = t("heatmap.cell", "{row} {column}: {value}", { row: r, column: c, value: formatValue(v) });
              return <span key={c} role="cell" aria-label={cell} title={cell} className="size-12 rounded-cell" style={{ background: RAMP[level(v)] }} />;
            })}
          </div>
        ))}
      </div>
      <div className="flex items-center gap-3 text-caption font-medium text-fg-tertiary" aria-hidden>
        <span className="mr-4">{t("heatmap.less", "Less")}</span>
        {RAMP.map((c) => (
          <span key={c} className="size-12 rounded-cell" style={{ background: c }} />
        ))}
        <span className="ml-4">{t("heatmap.more", "More")}</span>
      </div>
    </div>
  );
});
Heatmap.displayName = "Heatmap";
