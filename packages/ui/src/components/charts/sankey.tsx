import { forwardRef, useId, useMemo, useState } from "react";
import { sankey as d3sankey, sankeyLinkHorizontal, type SankeyLink, type SankeyNode } from "d3-sankey";
import { cn } from "../../utils/cn.js";
import { formatNumber } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import { ChartFrame, ChartLive, labelWidth, seriesColors, useChartKeys, useSize, type ChartState } from "./core.js";
import { fitLabel } from "./domain.js";

export const KAESO_SANKEY_VARIANTS = {
  size: { md: { classes: "", description: "Fills its width; 320px tall by default" } },
} as const;
export const KAESO_SANKEY_DEFAULT_VARIANTS = { size: "md" } as const;

export interface SankeyNodeIn {
  id: string;
  label: string;
}
export interface SankeyLinkIn {
  source: string;
  target: string;
  value: number;
}

type N = SankeyNode<SankeyNodeIn, SankeyLinkIn>;
type L = SankeyLink<SankeyNodeIn, SankeyLinkIn>;

const NODE = 8;
const LABEL_GAP = 8;
const COLUMN_GAP = 24;
const FONT = 13;
const width13 = (text: string) => labelWidth(text) * (FONT / 12);

export const Sankey = forwardRef<
  HTMLDivElement,
  {
    nodes: SankeyNodeIn[];
    links: SankeyLinkIn[];
    height?: number | undefined;
    formatValue?: ((v: number) => string) | undefined;
    "aria-label": string;
    className?: string | undefined;
  } & ChartState
>(function Sankey({ nodes, links, height = 320, formatValue = (v) => formatNumber(v), className, "aria-label": ariaLabel, ...state }, ref) {
  const t = useUiText();
  const [box, size] = useSize<HTMLDivElement>();
  const [hover, setHover] = useState<string | null>(null);
  const gid = useId().replace(/:/g, "");
  const flowing = links.filter((l) => Number.isFinite(l.value) && l.value > 0);
  const used = new Set(flowing.flatMap((l) => [l.source, l.target]));
  const shown = nodes.filter((n) => used.has(n.id));
  const empty = state.empty ?? flowing.length === 0;
  const palette = useMemo(() => seriesColors(nodes.map((n) => n.id)), [nodes]);
  const seriesColor = (id: string) => palette.get(id)!;
  const layout = useMemo(() => {
    if (!size.width || empty) return null;
    const graph = d3sankey<SankeyNodeIn, SankeyLinkIn>()
      .nodeId((d) => d.id)
      .nodeWidth(NODE)
      .nodePadding(16)
      .extent([
        [0, 4],
        [size.width, height - 4],
      ])({ nodes: shown.map((n) => ({ ...n })), links: flowing.map((l) => ({ ...l })) });
    const all = graph.nodes as N[];
    const last = Math.max(0, ...all.map((n) => n.depth ?? 0));
    const columns = Array.from({ length: last + 1 }, (_, d) => all.filter((n) => n.depth === d));
    const full = (n: N) => `${n.label} ${formatValue(n.value ?? 0)}`;
    const room = (withValues: boolean) => columns.map((col) => Math.max(0, ...col.map((n) => width13(withValues ? full(n) : n.label))) + LABEL_GAP);
    const need = (r: number[]) => (last + 1) * NODE + r.reduce((a, w) => a + w, 0) + Math.max(0, last - 1) * COLUMN_GAP + (last > 0 ? COLUMN_GAP : 0);
    let values = true;
    let widths = room(true);
    if (need(widths) > size.width) {
      values = false;
      widths = room(false);
    }
    const spare = size.width - need(widths);
    if (spare < 0) {
      const total = widths.reduce((a, w) => a + w, 0) || 1;
      widths = widths.map((w) => Math.max(LABEL_GAP + 24, w + (spare * w) / total));
    }
    const extra = last > 0 ? Math.max(0, spare) / last : 0;
    const x0 = columns.map(() => 0);
    for (let d = 1; d <= last; d++) {
      x0[d] = x0[d - 1]! + NODE + (d - 1 === last - 1 && last > 0 ? widths[d - 1]! + COLUMN_GAP + widths[d]! : widths[d - 1]! + COLUMN_GAP) + extra;
    }
    if (last === 0) x0[0] = 0;
    for (const n of all) {
      const d = n.depth ?? 0;
      n.x0 = Math.round(d === last && last > 0 ? Math.min(size.width - NODE, x0[d]!) : x0[d]!);
      n.x1 = n.x0 + NODE;
    }
    const placed = all.map((n) => {
      const d = n.depth ?? 0;
      const left = d === last && last > 0;
      const text = values ? full(n) : n.label;
      const fitted = fitLabel(n.label, widths[d]! - LABEL_GAP - (values ? width13(` ${formatValue(n.value ?? 0)}`) : 0), FONT);
      return { n, left, label: fitted, value: values ? formatValue(n.value ?? 0) : null, cut: fitted !== n.label || text !== full(n) };
    });
    return { graph, placed };
  }, [shown, flowing, size.width, height, empty, formatValue]);
  const table = {
    columns: [t("sankey.from", "From"), t("sankey.to", "To"), t("sankey.requests", "Requests")],
    rows: flowing.map((l) => [
      nodes.find((n) => n.id === l.source)?.label ?? l.source,
      nodes.find((n) => n.id === l.target)?.label ?? l.target,
      formatValue(l.value),
    ]),
  };
  const order = (layout?.placed ?? []).slice().sort((a, b) => (a.n.depth ?? 0) - (b.n.depth ?? 0) || (a.n.y0 ?? 0) - (b.n.y0 ?? 0));
  const quiet = empty || !!state.loading || !!state.error;
  const keys = useChartKeys(quiet ? 0 : order.length, { start: 0 });
  const keyed = keys.index !== null ? order[keys.index]?.n : undefined;
  const focus = hover ?? keyed?.id ?? null;
  const linkPath = sankeyLinkHorizontal<SankeyNodeIn, SankeyLinkIn>();
  const lit = (l: L) => !focus || (l.source as N).id === focus || (l.target as N).id === focus;
  return (
    <div ref={ref} className={cn("w-full min-w-0", className)}>
      <ChartFrame label={ariaLabel} height={height} table={table} state={{ ...state, empty }}>
        <div ref={box} role="group" aria-label={ariaLabel} {...keys.props} className="absolute inset-0 rounded-cell focus-ring">
          {layout && (
            <svg width={size.width} height={height} role="presentation" className="block">
              <defs>
                {(layout.graph.links as L[]).map((l, i) => (
                  <linearGradient key={i} id={`${gid}-${i}`} gradientUnits="userSpaceOnUse" x1={(l.source as N).x1} x2={(l.target as N).x0}>
                    <stop offset="0%" stopColor={seriesColor((l.source as N).id)} />
                    <stop offset="100%" stopColor={seriesColor((l.target as N).id)} />
                  </linearGradient>
                ))}
              </defs>
              <g>
                {(layout.graph.links as L[]).map((l, i) => (
                  <path
                    key={i}
                    d={linkPath(l) ?? ""}
                    fill="none"
                    stroke={`url(#${gid}-${i})`}
                    strokeOpacity={0.5}
                    opacity={focus ? (lit(l) ? 1 : 0.12) : 0.48}
                    strokeWidth={Math.max(1, l.width ?? 1)}
                    className="opacity-motion"
                  >
                    <title>
                      {t("sankey.link", "{source} → {target}: {value}", {
                        source: (l.source as N).label,
                        target: (l.target as N).label,
                        value: formatValue(l.value),
                      })}
                    </title>
                  </path>
                ))}
              </g>
              <g>
                {layout.placed.map(({ n, left, label, value, cut }) => {
                  const on = keyed?.id === n.id;
                  const y = ((n.y0 ?? 0) + (n.y1 ?? 0)) / 2;
                  return (
                    <g key={n.id} onMouseEnter={() => setHover(n.id)} onMouseLeave={() => setHover(null)} className="cursor-default">
                      {on && (
                        <rect
                          x={(n.x0 ?? 0) - 3}
                          y={(n.y0 ?? 0) - 3}
                          width={NODE + 6}
                          height={Math.max(2, (n.y1 ?? 0) - (n.y0 ?? 0)) + 6}
                          rx={6}
                          fill="none"
                          stroke="var(--k-text)"
                          strokeWidth={1.5}
                        />
                      )}
                      <rect x={n.x0} y={n.y0} width={NODE} height={Math.max(2, (n.y1 ?? 0) - (n.y0 ?? 0))} rx={4} fill={seriesColor(n.id)} />
                      <text
                        x={left ? (n.x0 ?? 0) - LABEL_GAP : (n.x1 ?? 0) + LABEL_GAP}
                        y={y}
                        dy="0.32em"
                        textAnchor={left ? "end" : "start"}
                        fill="var(--k-text)"
                        fontSize={FONT}
                        fontWeight={500}
                        stroke="var(--k-bg-panel)"
                        strokeWidth={3}
                        strokeLinejoin="round"
                        paintOrder="stroke"
                      >
                        {label}
                        {value !== null && (
                          <tspan fill="var(--k-text-tertiary)" dx={6} style={{ fontVariantNumeric: "tabular-nums" }}>
                            {value}
                          </tspan>
                        )}
                        {cut && <title>{`${n.label} ${formatValue(n.value ?? 0)}`}</title>}
                      </text>
                    </g>
                  );
                })}
              </g>
            </svg>
          )}
          <ChartLive>{keyed ? t("sankey.node", "{node}: {value}", { node: keyed.label, value: formatValue(keyed.value ?? 0) }) : null}</ChartLive>
        </div>
      </ChartFrame>
    </div>
  );
});
Sankey.displayName = "Sankey";
