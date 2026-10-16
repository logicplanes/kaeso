import { forwardRef, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { Icon, type IconData } from "../icon/icon";
import { useUiText } from "../../i18n/context";

export const KAESO_FLOW_VARIANTS = {
  node: {
    pill: { classes: "h-36 rounded-pill px-14", description: "A raised pill (default)" },
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

export const Flow = forwardRef<HTMLDivElement, FlowProps>(function Flow(
  { columns, edges, node = "pill", animated = false, className, "aria-label": ariaLabel },
  ref,
) {
  const t = useUiText();
  const root = useRef<HTMLDivElement | null>(null);
  const [paths, setPaths] = useState<Path[]>([]);
  const [size, setSize] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () => {
      const box = el.getBoundingClientRect();
      const pos = (id: string) => el.querySelector<HTMLElement>(`[data-flow-node="${id}"]`)?.getBoundingClientRect();
      setSize({ w: Math.round(box.width), h: Math.round(box.height) });
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
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(measure) : null;
    ro?.observe(el);
    document.fonts?.ready.then(measure);
    return () => ro?.disconnect();
  }, [columns, edges]);
  return (
    <div
      ref={(e) => {
        root.current = e;
        if (typeof ref === "function") ref(e);
        else if (ref) ref.current = e;
      }}
      role="img"
      aria-label={ariaLabel ?? t("flow.label", "Flow diagram")}
      className={cn("relative -m-4 inline-flex max-w-full-bleed-4 items-center gap-64 overflow-x-auto p-4 scrollbar-quiet", className)}
    >
      <svg aria-hidden className="pointer-events-none absolute inset-0 overflow-visible" width={size.w} height={size.h}>
        {paths.map((p) => (
          <g key={p.key}>
            <path d={p.d} fill="none" stroke="var(--k-line-strong)" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round" />
            {animated && (
              <circle r={2.5} fill="var(--k-text-secondary)">
                <animateMotion dur="2.4s" repeatCount="indefinite" path={p.d} />
              </circle>
            )}
          </g>
        ))}
      </svg>
      {columns.map((col, ci) => (
        <div key={ci} className="relative flex shrink-0 flex-col items-start gap-16">
          {col.map((n) => (
            <div
              key={n.id}
              data-flow-node={n.id}
              className={cn(
                "inline-flex items-center gap-8 bg-raised text-body font-medium text-fg",
                KAESO_FLOW_VARIANTS.node[node].classes,
                n.emphasis ? "ring-step" : "shadow-control",
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
  );
});
Flow.displayName = "Flow";
