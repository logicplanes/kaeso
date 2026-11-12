import { forwardRef, useEffect, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { plainFormatters, type Formatters } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";

export const KAESO_RESPONSE_METRICS_VARIANTS = {
  size: {
    sm: { classes: "text-caption gap-x-16 gap-y-2", description: "12/16: a column footer or under a turn (default)" },
    md: { classes: "text-small gap-x-20 gap-y-4", description: "13/18: a response on its own" },
  },
} as const;
export const KAESO_RESPONSE_METRICS_DEFAULT_VARIANTS = { size: "sm" } as const;

export interface ResponseMetricsProps {
  timeToFirstToken?: number | null | undefined;
  duration?: number | null | undefined;
  inputTokens?: number | null | undefined;
  outputTokens?: number | null | undefined;
  reasoningTokens?: number | null | undefined;
  live?: boolean | undefined;
  startedAt?: number | undefined;
  cost?: string | null | undefined;
  costKnown?: boolean | undefined;
  size?: keyof typeof KAESO_RESPONSE_METRICS_VARIANTS.size | undefined;
  className?: string | undefined;
}

export function formatLatency(ms: number, { formatDuration }: Formatters = plainFormatters()): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  return formatDuration(ms, { precise: true });
}

function useElapsed(from: number | undefined) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (from === undefined) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 100);
    return () => clearInterval(id);
  }, [from]);
  return from === undefined ? undefined : Math.max(0, now - from);
}

function requestCost(amount: string, { formatPrice, formatUSD }: Formatters): string {
  return /[1-9]/.test(amount) ? formatPrice(amount) : formatUSD("0", { decimals: 0 });
}

function known(n: number | null | undefined): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

function Item({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-6 whitespace-nowrap">
      <dt className="text-fg-tertiary">{label}</dt>
      <dd className="text-fg-secondary tabular">{children}</dd>
    </div>
  );
}

export const ResponseMetrics = forwardRef<HTMLDListElement, ResponseMetricsProps>(function ResponseMetrics(
  { timeToFirstToken, duration, inputTokens, outputTokens, reasoningTokens, live = false, startedAt, cost, costKnown = true, size = "sm", className },
  ref,
) {
  const t = useUiText();
  const f = useFormatters();
  const { formatDuration, formatNumber } = f;
  const counting = live && startedAt !== undefined && !known(duration);
  const elapsed = useElapsed(counting ? startedAt : undefined);
  const tokens = known(inputTokens) || known(outputTokens);
  const unknownCost = !costKnown && !live;
  const hasCost = unknownCost || (cost != null && cost !== "");
  const line = cn("flex min-w-0 flex-wrap items-baseline", KAESO_RESPONSE_METRICS_VARIANTS.size[size].classes, className);
  if (!tokens && !hasCost && !known(timeToFirstToken) && !known(duration) && !known(reasoningTokens) && elapsed === undefined) {
    return <dl ref={ref} aria-label={t("responseMetrics.metrics", "Metrics")} className={cn(line, size === "sm" ? "min-h-16" : "min-h-18")} />;
  }
  const seconds = (ms: number) =>
    ms < 10_000
      ? t("format.seconds", "{s} s", { s: formatNumber(Math.floor(ms / 100) / 10, { minimumFractionDigits: 1, maximumFractionDigits: 1 }) })
      : ms < 60_000
        ? t("format.seconds", "{s} s", { s: formatNumber(Math.floor(ms / 1000)) })
        : formatDuration(Math.floor(ms / 1000) * 1000);
  return (
    <dl ref={ref} aria-label={t("responseMetrics.metrics", "Metrics")} className={line}>
      {known(timeToFirstToken) && <Item label={t("responseMetrics.firstToken", "First token")}>{formatLatency(timeToFirstToken, f)}</Item>}
      {(known(duration) || elapsed !== undefined) && (
        <Item label={t("responseMetrics.total", "Total")}>
          {known(duration) ? formatLatency(duration, f) : <span className="text-fg-tertiary">{seconds(elapsed ?? 0)}</span>}
        </Item>
      )}
      {tokens && (
        <Item label={t("responseMetrics.tokens", "Tokens")}>
          {t("responseMetrics.tokensValue", "{input} in · {output} out", {
            input: known(inputTokens) ? formatNumber(inputTokens) : "—",
            output: known(outputTokens) ? formatNumber(outputTokens) : "—",
          })}
        </Item>
      )}
      {known(reasoningTokens) && <Item label={t("responseMetrics.reasoning", "Reasoning")}>{formatNumber(reasoningTokens)}</Item>}
      {hasCost && (
        <Item label={t("responseMetrics.cost", "Cost")}>{unknownCost ? t("responseMetrics.costUnknown", "Unknown") : requestCost(cost ?? "0", f)}</Item>
      )}
    </dl>
  );
});
ResponseMetrics.displayName = "ResponseMetrics";
