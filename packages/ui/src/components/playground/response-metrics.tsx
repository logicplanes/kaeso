import { forwardRef, useEffect, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatDuration, formatNumber, formatPrice, formatUSD } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_RESPONSE_METRICS_VARIANTS = {
  size: {
    sm: { classes: "text-caption gap-x-12 gap-y-2", description: "12/16: a column footer or under a turn (default)" },
    md: { classes: "text-small gap-x-16 gap-y-4", description: "13/18: a response on its own" },
  },
} as const;
export const KAESO_RESPONSE_METRICS_DEFAULT_VARIANTS = { size: "sm" } as const;

export interface ResponseMetricsProps {
  timeToFirstToken?: number | null | undefined;
  duration?: number | null | undefined;
  inputTokens?: number | null | undefined;
  outputTokens?: number | null | undefined;
  reasoningTokens?: number | null | undefined;
  reasoning?: boolean | undefined;
  live?: boolean | undefined;
  startedAt?: number | undefined;
  cost?: string | null | undefined;
  size?: keyof typeof KAESO_RESPONSE_METRICS_VARIANTS.size | undefined;
  className?: string | undefined;
}

export function formatLatency(ms: number): string {
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

function requestCost(amount: string): string {
  return /[1-9]/.test(amount) ? formatPrice(amount) : formatUSD("0", { decimals: 0 });
}

function known(n: number | null | undefined): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

function Item({ label, reserve, children }: { label: string; reserve: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-4 whitespace-nowrap">
      <dt className="text-fg-tertiary">{label}</dt>
      <dd className="inline-grid justify-items-start text-fg-secondary tabular">
        <span className="col-start-1 row-start-1">{children}</span>
        <span aria-hidden className="invisible col-start-1 row-start-1">
          {reserve}
        </span>
      </dd>
    </div>
  );
}

export const ResponseMetrics = forwardRef<HTMLDListElement, ResponseMetricsProps>(function ResponseMetrics(
  { timeToFirstToken, duration, inputTokens, outputTokens, reasoningTokens, reasoning, live = false, startedAt, cost, size = "sm", className },
  ref,
) {
  const t = useUiText();
  const counting = live && startedAt !== undefined && !known(duration);
  const elapsed = useElapsed(counting ? startedAt : undefined);
  const tokens = known(inputTokens) || known(outputTokens);
  const hasCost = cost != null && cost !== "";
  const line = cn("flex min-w-0 flex-wrap items-baseline", KAESO_RESPONSE_METRICS_VARIANTS.size[size].classes, className);
  if (!live && !tokens && !hasCost && !known(timeToFirstToken) && !known(duration) && !known(reasoningTokens)) {
    return <dl ref={ref} aria-label={t("responseMetrics.metrics", "Metrics")} className={cn(line, size === "sm" ? "min-h-16" : "min-h-18")} />;
  }
  const show = (value: boolean) => live || value;
  const dash = (
    <span className="text-fg-tertiary">
      <span aria-hidden>—</span>
      <span className="sr-only">{live ? t("responseMetrics.notYetKnown", "not yet known") : t("responseMetrics.notReported", "not reported")}</span>
    </span>
  );
  const seconds = (ms: number) =>
    ms < 10_000
      ? t("format.seconds", "{s} s", { s: formatNumber(Math.floor(ms / 100) / 10, { minimumFractionDigits: 1, maximumFractionDigits: 1 }) })
      : ms < 60_000
        ? t("format.seconds", "{s} s", { s: formatNumber(Math.floor(ms / 1000)) })
        : formatDuration(Math.floor(ms / 1000) * 1000);
  const time = formatLatency(8880);
  return (
    <dl ref={ref} aria-label={t("responseMetrics.metrics", "Metrics")} className={line}>
      {show(known(timeToFirstToken)) && (
        <Item label={t("responseMetrics.firstToken", "First token")} reserve={time}>
          {known(timeToFirstToken) ? formatLatency(timeToFirstToken) : dash}
        </Item>
      )}
      {show(known(duration)) && (
        <Item label={t("responseMetrics.total", "Total")} reserve={time}>
          {known(duration) ? formatLatency(duration) : elapsed !== undefined ? <span className="text-fg-tertiary">{seconds(elapsed)}</span> : dash}
        </Item>
      )}
      {show(tokens) && (
        <Item
          label={t("responseMetrics.tokens", "Tokens")}
          reserve={t("responseMetrics.tokensValue", "{input} in · {output} out", { input: formatNumber(8888), output: formatNumber(888) })}
        >
          {tokens
            ? t("responseMetrics.tokensValue", "{input} in · {output} out", {
                input: known(inputTokens) ? formatNumber(inputTokens) : "—",
                output: known(outputTokens) ? formatNumber(outputTokens) : "—",
              })
            : dash}
        </Item>
      )}
      {(known(reasoningTokens) || reasoning) && (
        <Item label={t("responseMetrics.reasoning", "Reasoning")} reserve={formatNumber(888)}>
          {known(reasoningTokens) ? formatNumber(reasoningTokens) : dash}
        </Item>
      )}
      {show(hasCost) && (
        <Item label={t("responseMetrics.cost", "Cost")} reserve={formatPrice("0.008888")}>
          {hasCost ? requestCost(cost) : dash}
        </Item>
      )}
    </dl>
  );
});
ResponseMetrics.displayName = "ResponseMetrics";
