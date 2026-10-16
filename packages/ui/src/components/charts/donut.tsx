import { forwardRef, useRef, useState, type ReactNode } from "react";
import { arc as d3arc, pie as d3pie } from "d3-shape";
import { cn } from "../../utils/cn";
import { formatNumber, formatPercent } from "../../format";
import { useLayoutWidth } from "../../hooks/use-layout-width";
import { englishText, useUiText, type UiText } from "../../i18n/context";
import { ChartFrame, Legend, Swatch, seriesColors, useTweened, type ChartState } from "./core";

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
  const data = groupSmall(slices, 0.02, t);
  const sum = data.reduce((a, s) => a + s.value, 0);
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
    rows: data.map((d) => [d.label, formatValue(d.value), formatPercent(sum ? d.value / sum : 0)]),
  };
  const box = useRef<HTMLDivElement | null>(null);
  const width = useLayoutWidth(box);
  const stacked = !!width && width < D + 40 + 220;
  return (
    <div
      ref={(el) => {
        box.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      className={cn("min-w-0", className)}
    >
      <ChartFrame
        label={ariaLabel}
        height={stacked ? D + 24 + data.length * 32 : Math.max(D, data.length * 32)}
        table={table}
        state={{ ...state, empty: state.empty ?? sum === 0 }}
      >
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
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-2">
              <span className={cn("font-semibold text-fg tabular", D >= 160 ? "text-title" : "text-title-sm")}>{total ?? formatValue(sum)}</span>
              <span className="text-caption font-medium text-fg-tertiary">{totalLabel ?? t("donut.total", "Total")}</span>
            </div>
          </div>
          <ul className={cn("flex min-w-0 flex-col", stacked ? "w-full" : "max-w-320 flex-1")} aria-label={t("donut.legend", "Legend")}>
            {data.map((d, i) => (
              <li
                key={d.id}
                onMouseEnter={() => setFocus(d.id)}
                onMouseLeave={() => setFocus(null)}
                className={cn(
                  "flex h-32 items-center gap-10 rounded-item px-8 text-small transition-control hover:bg-fill-1",
                  focus && focus !== d.id && "opacity-50",
                )}
              >
                <Swatch color={color(d)} />
                <span className="min-w-0 flex-1 truncate font-medium text-fg">{d.label}</span>
                <span className="text-fg-secondary tabular">{formatValue(d.value)}</span>
                <span className="w-40 text-right text-fg-tertiary tabular">{shares[i]}</span>
              </li>
            ))}
          </ul>
        </div>
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
  return (
    <div ref={ref} className={cn("flex flex-col gap-12", className)}>
      <div
        role="img"
        aria-label={t("shareBar.summary", "{label}: {shares}", {
          label: ariaLabel,
          shares: slices
            .map((s) => t("shareBar.share", "{label} {share}", { label: s.label, share: formatPercent(s.value / sum, 0) }))
            .join(t("shareBar.listSeparator", ", ")),
        })}
        className="flex h-8 w-full gap-2 overflow-hidden rounded-pill"
      >
        {slices
          .filter((s) => s.value > 0)
          .map((s) => (
            <span
              key={s.id}
              className="h-full min-w-2 first:rounded-l-pill last:rounded-r-pill width-motion"
              style={{
                width: `${(s.value / sum) * 100}%`,
                background: color(s),
              }}
              title={t("shareBar.segment", "{label}: {value}", { label: s.label, value: formatValue(s.value) })}
            />
          ))}
      </div>
      {legend && (
        <Legend
          items={slices.map((s) => ({
            id: s.id,
            label: s.label,
            color: color(s),
            value: formatPercent(s.value / sum, 0),
          }))}
        />
      )}
    </div>
  );
});
ShareBar.displayName = "ShareBar";
