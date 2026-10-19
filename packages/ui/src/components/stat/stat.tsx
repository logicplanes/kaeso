import {
  Children,
  cloneElement,
  createContext,
  forwardRef,
  isValidElement,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { Badge } from "../badge/badge.js";
import { Sparkline } from "../charts/sparkline.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { useLocale, useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_STAT_VARIANTS = {
  size: {
    md: { classes: "text-metric", description: "28/32 metric (default)" },
    sm: { classes: "text-title", description: "20/28 for dense rows" },
  },
} as const;
export const KAESO_STAT_DEFAULT_VARIANTS = { size: "md" } as const;

export interface StatProps {
  value: ReactNode;
  unit?: ReactNode | undefined;
  delta?: { label: string; tone?: "success" | "danger" | "neutral" | undefined } | undefined;
  size?: keyof typeof KAESO_STAT_VARIANTS.size | undefined;
  className?: string | undefined;
}

export const Stat = forwardRef<HTMLDivElement, StatProps>(function Stat({ value, unit, delta, size = "md", className }, ref) {
  return (
    <div ref={ref} className={cn("flex min-w-0 flex-wrap items-baseline gap-x-8 gap-y-4", className)}>
      <span className={cn("font-semibold text-fg tabular", KAESO_STAT_VARIANTS.size[size].classes)}>{value}</span>
      {unit && <span className="text-body-lg font-regular text-fg-secondary">{unit}</span>}
      {delta && (
        <Badge tone={delta.tone ?? "neutral"} className="self-center">
          {delta.label}
        </Badge>
      )}
    </div>
  );
});
Stat.displayName = "Stat";

export const MetricRow = forwardRef<HTMLDivElement, { children: ReactNode; className?: string | undefined }>(function MetricRow({ children, className }, ref) {
  return (
    <div ref={ref} className={cn("flex flex-wrap items-baseline gap-x-48 gap-y-16", className)}>
      {children}
    </div>
  );
});
MetricRow.displayName = "MetricRow";

export interface StatTileProps {
  label: ReactNode;
  value: ReactNode;
  change?: { value: number; better?: "up" | "down" | undefined } | undefined;
  detail?: ReactNode | undefined;
  trend?: number[] | undefined;
  trendColor?: string | undefined;
  loading?: boolean | undefined;
  className?: string | undefined;
}

const changeLabel = (v: number, t: UiText, locale: string) => {
  const pct = Math.abs(v) * 100;
  const digits = pct >= 10 || pct === 0 ? 0 : 1;
  const shown = new Intl.NumberFormat(locale === "en" ? "en-US" : locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(digits ? Number(pct.toFixed(1)) : Math.round(pct));
  return `${v > 0 ? "↑" : v < 0 ? "↓" : ""}${t("statTile.percent", "{value}%", { value: shown })}`;
};

export const STAT_VALUE_STEPS = [
  { classes: "text-title-lg", px: 28 },
  { classes: "text-title", px: 20 },
  { classes: "text-title-sm", px: 17 },
] as const;

const StatStepContext = createContext(0);

export const StatTile = forwardRef<HTMLDivElement, StatTileProps>(function StatTile(
  { label, value, change, detail, trend, trendColor, loading, className },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const good = change && change.value !== 0 ? change.value > 0 === ((change.better ?? "up") === "up") : undefined;
  const step = STAT_VALUE_STEPS[useContext(StatStepContext)] ?? STAT_VALUE_STEPS[0];
  return (
    <div
      ref={ref}
      role="listitem"
      aria-busy={loading || undefined}
      className={cn("flex min-w-0 flex-col bg-raised px-20 pt-16", trend || loading ? "pb-12" : "pb-16", className)}
    >
      <span className="text-small font-medium text-fg-secondary">{label}</span>
      <span className="mt-6 flex h-34 min-w-0 items-center">
        {loading ? (
          <Skeleton shape="line" className="h-20 w-96" />
        ) : (
          <span data-stat-value={step.px} className={cn("max-w-full truncate font-semibold text-fg tabular", step.classes)}>
            {value}
          </span>
        )}
      </span>
      {loading ? (
        <>
          <span className="mt-2 flex h-20 items-center">
            <Skeleton shape="line" className="w-120" />
          </span>
          <span className="mt-auto h-40 shrink-0 pt-8">
            <Skeleton className="h-full w-full" />
          </span>
        </>
      ) : (
        <span className="mt-2 flex min-h-20 min-w-0 flex-wrap items-center gap-x-6 gap-y-2 text-small text-fg-tertiary">
          {change && Number.isFinite(change.value) && (
            <span className="flex h-20 shrink-0 items-center">
              <Badge tone={good === undefined ? "neutral" : good ? "success" : "danger"} className="shrink-0 tabular">
                {changeLabel(change.value, t, locale)}
              </Badge>
            </span>
          )}
          {detail && <span className="min-w-0">{detail}</span>}
        </span>
      )}
      {trend && !loading && (
        <span className="mt-auto h-40 shrink-0 pt-8">
          {trend.length > 1 && (
            <Sparkline
              values={trend}
              fluid
              {...(trendColor ? { color: trendColor } : {})}
              aria-label={typeof label === "string" ? t("statTile.trendPerDay", "{label} per day", { label }) : t("statTile.trend", "Trend")}
            />
          )}
        </span>
      )}
    </div>
  );
});
StatTile.displayName = "StatTile";

const GRID_COLS: Record<number, string> = { 1: "grid-cols-1", 2: "grid-cols-2" };
const LG_GRID_COLS: Record<number, string> = { 3: "lg:grid-cols-3", 4: "lg:grid-cols-4" };
const COL_SPAN: Record<number, string> = { 2: "col-span-2" };
const LG_COL_SPAN: Record<number, string> = { 1: "lg:col-span-1", 2: "lg:col-span-2", 3: "lg:col-span-3", 4: "lg:col-span-4" };

const TILE_PADDING = 40;
const GAP = 1;
const SLACK = 1.03;

export function statGridLayout(n: number, wide: boolean): { cols: number; spans: number[] } {
  const cols = n <= 2 ? Math.max(n, 1) : wide ? Math.min(n, 4) : 2;
  const last = n - 1;
  const spans = Array.from({ length: n }, (_, i) => {
    if (i !== last || n < 3) return 1;
    if (n === 3) return wide ? 1 : 2;
    return wide ? ({ 1: 4, 2: 3, 3: 2 }[n % 4] ?? 1) : n % 2 === 1 ? 2 : 1;
  });
  return { cols, spans };
}

const MIN_TILE = 160;

export function statGridFit(needs: number[], width: number, wide: boolean): { step: number; single: boolean; two: boolean } {
  const base = STAT_VALUE_STEPS[0].px;
  const at = ({ cols, spans }: { cols: number; spans: number[] }) => {
    const col = (width - (cols - 1) * GAP) / cols;
    const rooms = spans.map((s) => col * s + (s - 1) * GAP - TILE_PADDING);
    return STAT_VALUE_STEPS.findIndex((st) => needs.every((need, i) => (need * st.px * SLACK) / base <= (rooms[i] ?? 0)));
  };
  const own = statGridLayout(needs.length, wide);
  const two = own.cols > 2 && (width - (own.cols - 1) * GAP) / own.cols < MIN_TILE;
  const step = at(two ? statGridLayout(needs.length, false) : own);
  if (step >= 0 || needs.length < 2) return { step: Math.max(step, 0), single: false, two };
  const single = at({ cols: 1, spans: needs.map(() => 1) });
  return { step: single >= 0 ? single : STAT_VALUE_STEPS.length - 1, single: true, two: false };
}

export const StatGrid = forwardRef<HTMLDivElement, { children: ReactNode; "aria-label"?: string | undefined; className?: string | undefined }>(
  function StatGrid({ children, className, ...props }, ref) {
    const tiles = Children.toArray(children).filter(isValidElement) as ReactElement<{ className?: string | undefined }>[];
    const n = tiles.length;
    const grid = useRef<HTMLDivElement | null>(null);
    const [fit, setFit] = useState({ step: 0, single: false, two: false });
    const measure = useRef(() => {});
    measure.current = () => {
      const el = grid.current;
      if (!el || !el.clientWidth) return;
      const values = [...el.querySelectorAll<HTMLElement>("[data-stat-value]")];
      const needs = values.map((v) => (v.scrollWidth * STAT_VALUE_STEPS[0].px) / (Number(v.dataset.statValue) || STAT_VALUE_STEPS[0].px));
      const wide = typeof window.matchMedia === "function" && window.matchMedia("(min-width: 1024px)").matches;
      const next = statGridFit(needs, el.clientWidth, wide);
      setFit((f) => (f.step === next.step && f.single === next.single && f.two === next.two ? f : next));
    };
    useLayoutEffect(() => measure.current());
    useLayoutEffect(() => {
      const el = grid.current;
      if (!el || typeof ResizeObserver === "undefined") return;
      const stop = observeResize([el, ...el.querySelectorAll("[data-stat-value]")], () => measure.current());
      void document.fonts?.ready.then(() => measure.current());
      return stop;
    }, [n]);
    const narrow = statGridLayout(n, false);
    const wide = statGridLayout(n, true);
    const span = (i: number) => {
      const small = narrow.spans[i] ?? 1;
      const large = wide.spans[i] ?? 1;
      return cn(COL_SPAN[small], !fit.two && large !== small && LG_COL_SPAN[large]) || undefined;
    };
    return (
      <div
        ref={(e) => {
          grid.current = e;
          if (typeof ref === "function") ref(e);
          else if (ref) ref.current = e;
        }}
        role="list"
        className={cn(
          "grid gap-1 overflow-hidden rounded-group bg-line-subtle shadow-xs",
          fit.single ? "grid-cols-1" : fit.two ? "grid-cols-2" : cn(GRID_COLS[narrow.cols], wide.cols !== narrow.cols && LG_GRID_COLS[wide.cols]),
          className,
        )}
        {...props}
      >
        <StatStepContext.Provider value={fit.step}>
          {tiles.map((t, i) => {
            const extra = fit.single ? undefined : span(i);
            return extra ? cloneElement(t, { className: cn(t.props.className, extra) }) : t;
          })}
        </StatStepContext.Provider>
      </div>
    );
  },
);
StatGrid.displayName = "StatGrid";
