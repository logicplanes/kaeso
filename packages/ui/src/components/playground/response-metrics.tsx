import { forwardRef, useEffect, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatDuration, formatNumber, formatUSD } from "../../format/index.js";
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

function known(n: number | null | undefined): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

function Item({ label, width, children, value }: { label: string; width: string; children: ReactNode; value?: string | undefined }) {
  return (
    <div className="flex items-baseline gap-4 whitespace-nowrap">
      <dt className="text-fg-tertiary">{label}</dt>
      <dd className={cn("text-fg-secondary tabular", width)} {...(value ? { "aria-label": value } : {})}>
        {children}
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
      <span className="sr-only">{t("responseMetrics.notYetKnown", "not yet known")}</span>
    </span>
  );
  return (
    <dl ref={ref} aria-label={t("responseMetrics.metrics", "Metrics")} className={line}>
      {show(known(timeToFirstToken)) && (
        <Item label={t("responseMetrics.firstToken", "First token")} width="min-w-40">
          {known(timeToFirstToken) ? formatLatency(timeToFirstToken) : dash}
        </Item>
      )}
      {show(known(duration)) && (
        <Item label={t("responseMetrics.total", "Total")} width="min-w-40">
          {known(duration) ? formatLatency(duration) : elapsed !== undefined ? <span className="text-fg-tertiary">{formatLatency(elapsed)}</span> : dash}
        </Item>
      )}
      {show(tokens) && (
        <Item
          label={t("responseMetrics.tokens", "Tokens")}
          width="min-w-48"
          value={
            tokens
              ? t("responseMetrics.tokensInOut", "{input} in, {output} out", {
                  input: known(inputTokens) ? formatNumber(inputTokens) : t("responseMetrics.unknown", "unknown"),
                  output: known(outputTokens) ? formatNumber(outputTokens) : t("responseMetrics.unknown", "unknown"),
                })
              : undefined
          }
        >
          {tokens ? (
            <>
              {known(inputTokens) ? formatNumber(inputTokens) : dash} → {known(outputTokens) ? formatNumber(outputTokens) : dash}
            </>
          ) : (
            dash
          )}
        </Item>
      )}
      {(known(reasoningTokens) || (live && reasoning)) && (
        <Item label={t("responseMetrics.reasoning", "Reasoning")} width="min-w-24">
          {known(reasoningTokens) ? formatNumber(reasoningTokens) : dash}
        </Item>
      )}
      {show(hasCost) && (
        <Item label={t("responseMetrics.cost", "Cost")} width="min-w-40">
          {hasCost ? formatUSD(cost) : dash}
        </Item>
      )}
    </dl>
  );
});
ResponseMetrics.displayName = "ResponseMetrics";
