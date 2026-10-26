import { forwardRef, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { motion } from "@logicplanes/kaeso-tokens";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { useScrollFadeX } from "../../hooks/use-scroll-fade.js";
import { Icon, type IconData } from "../icon/icon.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_FLOW_VARIANTS = {
  node: {
    pill: { classes: "h-36 rounded-pill px-14", description: "A pill with a hairline (default)" },
    box: { classes: "min-h-56 rounded-well px-16 py-10", description: "A rounded-14 box with a description line" },
  },
} as const;
export const KAESO_FLOW_DEFAULT_VARIANTS = { node: "pill" } as const;

export interface FlowNode {
  id: string;
  label: ReactNode;
  description?: ReactNode | undefined;
  icon?: IconData | undefined;
  leading?: ReactNode | undefined;
  emphasis?: boolean | undefined;
}

export interface FlowProps {
  columns: FlowNode[][];
  edges: [string, string][];
  node?: keyof typeof KAESO_FLOW_VARIANTS.node | undefined;
  animated?: boolean | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

interface Path {
  d: string;
  key: string;
}

const routeKey = (edges: [string, string][]) => edges.map(([a, b]) => `${a}>${b}`).join("|");
const reducedMotion = () => typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
const PASS = motion.duration.fresh;

function useRoutePass(route: string, enabled: boolean) {
  const seen = useRef(route);
  const [progress, setProgress] = useState<number | null>(null);
  useEffect(() => {
    if (seen.current === route) return;
    seen.current = route;
    if (!enabled || reducedMotion()) return;
    const start = performance.now();
    let raf = 0;
    const step = () => {
      const t = (performance.now() - start) / PASS;
      setProgress(t >= 1 ? null : t);
      if (t < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => {
      cancelAnimationFrame(raf);
      setProgress(null);
    };
  }, [route, enabled]);
  return progress;
}

export const Flow = forwardRef<HTMLDivElement, FlowProps>(function Flow(
  { columns, edges, node = "pill", animated = false, className, "aria-label": ariaLabel },
  ref,
) {
  const t = useUiText();
  const [beforeFrom, fromRest = ""] = t("flow.connection", "{from} to {to}").split("{from}");
  const [beforeTo, afterTo = ""] = fromRest.split("{to}");
  const labels = new Map(columns.flat().map((n) => [n.id, n.label]));
  const connections = edges.filter(([a, b]) => labels.has(a) && labels.has(b));
  const root = useRef<HTMLDivElement | null>(null);
  const content = useRef<HTMLDivElement | null>(null);
  const lines = useRef<SVGSVGElement | null>(null);
  const [paths, setPaths] = useState<Path[]>([]);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [scrolls, setScrolls] = useState(false);
  useScrollFadeX(root, "[data-flow-content]");
  const progress = useRoutePass(routeKey(connections), animated);
  useLayoutEffect(() => {
    const el = content.current;
    const outer = root.current;
    if (!el || !outer) return;
    const measure = () => {
      const box = el.getBoundingClientRect();
      const pos = (id: string) => el.querySelector<HTMLElement>(`[data-flow-node="${id}"]`)?.getBoundingClientRect();
      setSize({ w: Math.round(box.width), h: Math.round(box.height) });
      setScrolls(outer.scrollWidth > outer.clientWidth + 1);
      setPaths(
        edges.flatMap(([a, b]) => {
          const p = pos(a);
          const q = pos(b);
          if (!p || !q) return [];
          const x1 = Math.round(p.right - box.left);
          const y1 = Math.round(p.top + p.height / 2 - box.top) + 0.5;
          const x2 = Math.round(q.left - box.left);
          const y2 = Math.round(q.top + q.height / 2 - box.top) + 0.5;
          const mx = Math.round((x1 + x2) / 2);
          const r = Math.min(12, Math.abs(y2 - y1) / 2, (x2 - x1) / 4);
          if (Math.abs(y2 - y1) < 1) return [{ key: `${a}-${b}`, d: `M${x1} ${y1}H${x2}` }];
          const dir = y2 > y1 ? 1 : -1;
          return [{ key: `${a}-${b}`, d: `M${x1} ${y1}H${mx - r}Q${mx} ${y1} ${mx} ${y1 + dir * r}V${y2 - dir * r}Q${mx} ${y2} ${mx + r} ${y2}H${x2}` }];
        }),
      );
    };
    measure();
    const stop = observeResize([outer, el], measure);
    document.fonts?.ready.then(measure);
    return stop;
  }, [columns, edges]);
  const dots =
    progress === null
      ? []
      : paths.flatMap((p) => {
          const line = lines.current?.querySelector<SVGPathElement>(`[data-flow-edge="${p.key}"]`);
          if (!line?.getTotalLength) return [];
          const at = line.getPointAtLength(line.getTotalLength() * progress);
          return [{ key: p.key, x: at.x, y: at.y }];
        });
  return (
    <div
      ref={(e) => {
        root.current = e;
        if (typeof ref === "function") ref(e);
        else if (ref) ref.current = e;
      }}
      role="figure"
      aria-label={ariaLabel ?? t("flow.label", "Flow diagram")}
      {...(scrolls ? { tabIndex: 0 } : {})}
      className={cn("relative inline-block max-w-full overflow-x-auto overflow-y-hidden align-top scrollbar-quiet scroll-fade-x focus-ring-inset", className)}
    >
      <div ref={content} data-flow-content="" className="relative flex w-max items-center gap-64">
        <svg ref={lines} aria-hidden className="pointer-events-none absolute inset-0 overflow-visible" width={size.w} height={size.h}>
          {paths.map((p) => (
            <path
              key={p.key}
              data-flow-edge={p.key}
              d={p.d}
              fill="none"
              stroke="var(--k-text-tertiary)"
              strokeWidth={1.5}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          ))}
          {dots.map((dot) => (
            <circle key={dot.key} data-flow-dot="" cx={dot.x} cy={dot.y} r={2.5} fill="var(--k-text-secondary)" />
          ))}
        </svg>
        {columns.map((col, ci) => (
          <div key={ci} className="relative flex shrink-0 flex-col items-start gap-16">
            {col.map((n) => (
              <div
                key={n.id}
                data-flow-node={n.id}
                className={cn(
                  "inline-flex items-center gap-8 bg-panel text-body font-medium text-fg",
                  KAESO_FLOW_VARIANTS.node[node].classes,
                  n.emphasis ? "ring-step forced-colors:outline-2 forced-colors:-outline-offset-2" : "shadow-ring",
                )}
              >
                {n.leading}
                {n.icon && <Icon icon={n.icon} size="sm" className="text-fg-secondary" />}
                <span className="flex flex-col whitespace-nowrap">
                  <span>{n.label}</span>
                  {n.description && node === "box" && <span className="text-small font-regular text-fg-secondary">{n.description}</span>}
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
      {connections.length > 0 && (
        <ul className="sr-only">
          {connections.map(([a, b]) => (
            <li key={`${a}-${b}`}>
              {beforeFrom}
              {labels.get(a)}
              {beforeTo}
              {labels.get(b)}
              {afterTo}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
});
Flow.displayName = "Flow";
