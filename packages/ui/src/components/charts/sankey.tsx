import { forwardRef, useId, useMemo, useState } from "react";
import { sankey as d3sankey, sankeyLinkHorizontal, type SankeyLink, type SankeyNode } from "d3-sankey";
import { cn } from "../../utils/cn.js";
import { formatNumber } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import { ChartFrame, seriesColors, useSize, type ChartState } from "./core.js";

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
  const [focus, setFocus] = useState<string | null>(null);
  const gid = useId().replace(/:/g, "");
  const palette = useMemo(() => seriesColors(nodes.map((n) => n.id)), [nodes]);
  const seriesColor = (id: string) => palette.get(id)!;
  const graph = useMemo(() => {
    if (!size.width) return null;
    const layout = d3sankey<SankeyNodeIn, SankeyLinkIn>()
      .nodeId((d) => d.id)
      .nodeWidth(8)
      .nodePadding(16)
      .extent([
        [0, 4],
        [size.width - 140, height - 4],
      ]);
    return layout({ nodes: nodes.map((n) => ({ ...n })), links: links.map((l) => ({ ...l })) });
  }, [nodes, links, size.width, height]);
  const table = {
    columns: [t("sankey.from", "From"), t("sankey.to", "To"), t("sankey.requests", "Requests")],
    rows: links.map((l) => [
      nodes.find((n) => n.id === l.source)?.label ?? l.source,
      nodes.find((n) => n.id === l.target)?.label ?? l.target,
      formatValue(l.value),
    ]),
  };
  const linkPath = sankeyLinkHorizontal<SankeyNodeIn, SankeyLinkIn>();
  const lit = (l: L) => !focus || (l.source as N).id === focus || (l.target as N).id === focus;
  return (
    <div ref={ref} className={cn("min-w-0", className)}>
      <ChartFrame label={ariaLabel} height={height} table={table} state={state}>
        <div ref={box} className="absolute inset-0">
          {graph && (
            <svg width={size.width} height={height} role="presentation" className="overflow-visible">
              <defs>
                {(graph.links as L[]).map((l, i) => (
                  <linearGradient key={i} id={`${gid}-${i}`} gradientUnits="userSpaceOnUse" x1={(l.source as N).x1} x2={(l.target as N).x0}>
                    <stop offset="0%" stopColor={seriesColor((l.source as N).id)} />
                    <stop offset="100%" stopColor={seriesColor((l.target as N).id)} />
                  </linearGradient>
                ))}
              </defs>
              <g>
                {(graph.links as L[]).map((l, i) => (
                  <path
                    key={i}
                    d={linkPath(l) ?? ""}
                    fill="none"
                    stroke={`url(#${gid}-${i})`}
                    strokeOpacity={focus ? (lit(l) ? 0.5 : 0.06) : 0.24}
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
                {(graph.nodes as N[]).map((n) => (
                  <g key={n.id} onMouseEnter={() => setFocus(n.id)} onMouseLeave={() => setFocus(null)} className="cursor-default">
                    <rect x={n.x0} y={n.y0} width={(n.x1 ?? 0) - (n.x0 ?? 0)} height={Math.max(2, (n.y1 ?? 0) - (n.y0 ?? 0))} rx={4} fill={seriesColor(n.id)} />
                    <text x={(n.x1 ?? 0) + 10} y={((n.y0 ?? 0) + (n.y1 ?? 0)) / 2} dy="0.32em" fill="var(--k-text)" fontSize={13} fontWeight={500}>
                      {n.label}
                      <tspan fill="var(--k-text-tertiary)" dx={6} style={{ fontVariantNumeric: "tabular-nums" }}>
                        {formatValue(n.value ?? 0)}
                      </tspan>
                    </text>
                  </g>
                ))}
              </g>
            </svg>
          )}
        </div>
      </ChartFrame>
    </div>
  );
});
Sankey.displayName = "Sankey";
