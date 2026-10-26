import { forwardRef, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { arc as d3arc, pie as d3pie } from "d3-shape";
import { cn } from "../../utils/cn.js";
import { formatNumber, formatPercent } from "../../format/index.js";
import { useLayoutWidth } from "../../hooks/use-layout-width.js";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";
import { ChartFrame, Legend, Swatch, seriesColors, useTweened, type ChartState } from "./core.js";
import { Text } from "../text/text.js";
import { edgeColor } from "./marks.js";

export interface Slice {
  id: string;
  label: string;
  value: number;
  color?: string | undefined;
}

export const KAESO_DONUT_VARIANTS = {
  size: {
    md: { classes: "size-160", description: "160px (default)" },
    sm: { classes: "size-140", description: "140px" },
    lg: { classes: "size-180", description: "180px" },
  },
} as const;
export const KAESO_DONUT_DEFAULT_VARIANTS = { size: "md" } as const;

export function groupSmall(slices: Slice[], min = 0.02, t: UiText = englishText): Slice[] {
  const total = slices.reduce((a, s) => a + s.value, 0) || 1;
  const small = slices.filter((s) => s.value > 0 && s.value / total < min);
  const big = slices.filter((s) => s.value / total >= min);
  if (small.length < 2) return [...big, ...small].sort((a, b) => slices.indexOf(a) - slices.indexOf(b));
  const rest = small.reduce((a, s) => a + s.value, 0);
  return [
    ...big,
    {
      id: "__other",
      label: t("donut.other", "Other"),
      value: rest,
      color: "var(--k-text-tertiary)",
    },
  ];
}

export function shareLabel(value: number, sum: number, t: UiText = englishText): string {
  const share = sum ? value / sum : 0;
  return share > 0 && share < 0.005 ? t("donut.underOnePercent", "<1%") : formatPercent(share, 0);
}

export function wholeShares(values: number[], t: UiText = englishText): string[] {
  const sum = values.reduce((a, v) => a + v, 0);
  if (!sum) return values.map(() => formatPercent(0, 0));
  const exact = values.map((v) => (v / sum) * 100);
  const floors = exact.map(Math.floor);
  let left = 100 - floors.reduce((a, v) => a + v, 0);
  const order = exact.map((e, i) => ({ i, r: e - Math.floor(e) })).sort((a, b) => b.r - a.r);
  for (const { i } of order) {
    if (left <= 0) break;
    floors[i] = (floors[i] ?? 0) + 1;
    left--;
  }
  return floors.map((f, i) => (f === 0 && (values[i] ?? 0) > 0 ? t("donut.underOnePercent", "<1%") : formatPercent(f / 100, 0)));
}

function useOtherData(identity: string): boolean {
  const [seen, setSeen] = useState(identity);
  if (seen !== identity) setSeen(identity);
  return seen !== identity;
}

const MAX_SLICES = 8;

function capSlices(slices: Slice[], t: UiText): Slice[] {
  if (slices.length <= MAX_SLICES) return slices;
  const named = slices.filter((s) => s.id !== "__other");
  const kept = named.slice(0, MAX_SLICES - 1);
  const rest = slices.filter((s) => !kept.includes(s)).reduce((a, s) => a + s.value, 0);
  return [...kept, { id: "__other", label: t("donut.other", "Other"), value: rest, color: "var(--k-text-tertiary)" }];
}

const TOTAL_SIZES = ["text-title", "text-title-sm", "text-body-lg"] as const;

function useFittedSize(text: string, room: number, first: number) {
  const ref = useRef<HTMLSpanElement | null>(null);
  const [step, setStep] = useState({ text, room, at: first });
  const current = step.text === text && step.room === room ? step : { text, room, at: first };
  if (current !== step) setStep(current);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || current.at >= TOTAL_SIZES.length - 1) return;
    if (el.scrollWidth > room) setStep({ text, room, at: current.at + 1 });
  }, [current.at, room, text]);
  return [ref, TOTAL_SIZES[current.at]!] as const;
}

export const Donut = forwardRef<
  HTMLDivElement,
  {
    slices: Slice[];
    size?: keyof typeof KAESO_DONUT_VARIANTS.size | undefined;
    total?: ReactNode | undefined;
    totalLabel?: ReactNode | undefined;
    formatValue?: ((v: number) => string) | undefined;
    "aria-label": string;
    className?: string | undefined;
  } & ChartState
>(function Donut({ slices, size = "md", total, totalLabel, formatValue = (v) => formatNumber(v), className, "aria-label": ariaLabel, ...state }, ref) {
  const t = useUiText();
  const D = { sm: 140, md: 160, lg: 180 }[size];
  const STROKE = 14;
  const data = capSlices(groupSmall(slices, 0.02, t), t);
  const sum = data.reduce((a, s) => a + s.value, 0);
  const empty = state.empty ?? sum === 0;
  const other = useOtherData(data.map((d) => d.id).join("\u0000"));
  const tween = useTweened(
    data.map((d) => d.value),
    !!state.loading || other,
  );
  const [focus, setFocus] = useState<string | null>(null);
  const mid = D / 2 - STROKE / 2;
  const pie = d3pie<number>()
    .sort(null)
    .padAngle(3 / mid)
    .value((v) => v);
  const arcs = pie(tween);
  const arc = d3arc<{
    startAngle: number;
    endAngle: number;
    padAngle: number;
  }>()
    .innerRadius(D / 2 - STROKE)
    .outerRadius(D / 2)
    .padRadius(mid);
  const palette = seriesColors(data.map((d) => d.id));
  const color = (s: Slice) => s.color ?? palette.get(s.id)!;
  const shares = wholeShares(
    data.map((d) => d.value),
    t,
  );
  const table = {
    columns: [t("donut.slice", "Slice"), t("donut.value", "Value"), t("donut.share", "Share")],
    rows: data.map((d, i) => [d.label, formatValue(d.value), shares[i] ?? ""]),
  };
  const box = useRef<HTMLDivElement | null>(null);
  const width = useLayoutWidth(box);
  const stacked = !!width && width < D + 40 + 220;
  const totalText = typeof total === "string" ? total : total === undefined ? formatValue(sum) : "";
  const [totalRef, totalSize] = useFittedSize(totalText, D - STROKE * 2 - 16, D >= 160 ? 0 : 1);
  return (
    <div
      ref={(el) => {
        box.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      className={cn("w-full min-w-0", className)}
    >
      <ChartFrame label={ariaLabel} height={stacked ? D + 24 + data.length * 32 : Math.max(D, data.length * 32)} table={table} state={{ ...state, empty }}>
        {!empty && (
          <div className={cn("flex h-full", stacked ? "flex-col items-center gap-24" : "items-center gap-40")}>
            <div className="relative shrink-0" style={{ width: D, height: D }}>
              <svg width={D} height={D} role="presentation">
                <g transform={`translate(${D / 2},${D / 2})`}>
                  {arcs.map((a, i) => {
                    const len = (a.endAngle - a.startAngle - a.padAngle) * mid;
                    return (
                      <path
                        key={data[i]!.id}
                        d={arc.cornerRadius(Math.max(0, Math.min(STROKE / 2, len / 2)))(a) ?? ""}
                        fill={color(data[i]!)}
                        opacity={focus && focus !== data[i]!.id ? 0.3 : 1}
                        className="opacity-motion"
                      />
                    );
                  })}
                </g>
              </svg>
              <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-16 text-center">
                <span ref={totalRef} className={cn("max-w-full font-semibold text-balance text-fg tabular wrap-anywhere", totalSize)}>
                  {total ?? formatValue(sum)}
                </span>
                <span className="text-caption font-medium text-fg-tertiary">{totalLabel ?? t("donut.total", "Total")}</span>
              </div>
            </div>
            <ul className={cn("flex min-w-0 flex-col", stacked ? "w-full" : "min-w-160 max-w-320 flex-1")} aria-label={t("donut.legend", "Legend")}>
              {data.map((d, i) => (
                <li
                  key={d.id}
                  onMouseEnter={() => setFocus(d.id)}
                  onMouseLeave={() => setFocus(null)}
                  className={cn("flex h-32 items-center gap-10 text-small opacity-motion", focus && focus !== d.id && "opacity-50")}
                >
                  <Swatch color={color(d)} />
                  <Text variant="small" weight="medium" truncate className="flex-1">
                    {d.label}
                  </Text>
                  <span className="text-fg-secondary tabular">{formatValue(d.value)}</span>
                  <span className="w-40 text-right text-fg-tertiary tabular">{shares[i]}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </ChartFrame>
    </div>
  );
});
Donut.displayName = "Donut";
export const Ring = Donut;

export const KAESO_SHARE_BAR_VARIANTS = {
  size: { md: { classes: "h-8", description: "One 8px pill (default)" } },
} as const;
export const KAESO_SHARE_BAR_DEFAULT_VARIANTS = { size: "md" } as const;

export const ShareBar = forwardRef<
  HTMLDivElement,
  {
    slices: Slice[];
    formatValue?: ((v: number) => string) | undefined;
    legend?: boolean | undefined;
    "aria-label": string;
    className?: string | undefined;
  }
>(function ShareBar({ slices, formatValue = (v) => formatNumber(v), legend = true, className, "aria-label": ariaLabel }, ref) {
  const t = useUiText();
  const sum = slices.reduce((a, s) => a + s.value, 0) || 1;
  const palette = seriesColors(slices.map((s) => s.id));
  const color = (s: Slice) => s.color ?? palette.get(s.id)!;
  const shares = wholeShares(
    slices.map((s) => s.value),
    t,
  );
  return (
    <div ref={ref} className={cn("flex flex-col gap-12", className)}>
      <div
        role="img"
        aria-label={t("shareBar.summary", "{label}: {shares}", {
          label: ariaLabel,
          shares: slices
            .map((s, i) => t("shareBar.valueShare", "{label} {value} ({share})", { label: s.label, value: formatValue(s.value), share: shares[i] ?? "" }))
            .join(t("shareBar.listSeparator", ", ")),
        })}
        className={cn("flex h-8 w-full gap-2 overflow-hidden rounded-pill", !slices.some((s) => s.value > 0) && "bg-fill-2")}
      >
        {slices
          .filter((s) => s.value > 0)
          .map((s) => {
            const edge = edgeColor(color(s));
            return (
              <span
                key={s.id}
                className="h-full min-w-2 data-mark first:rounded-l-pill last:rounded-r-pill width-motion"
                style={{
                  width: `${(s.value / sum) * 100}%`,
                  background: color(s),
                  ...(edge ? { boxShadow: `inset 0 0 0 1px ${edge}` } : {}),
                }}
              />
            );
          })}
      </div>
      {legend && (
        <Legend
          items={slices.map((s, i) => ({
            id: s.id,
            label: s.label,
            color: color(s),
            value: shares[i],
          }))}
        />
      )}
    </div>
  );
});
ShareBar.displayName = "ShareBar";
