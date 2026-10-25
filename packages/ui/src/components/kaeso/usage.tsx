import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { formatNumber, formatTokens } from "../../format/index.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Badge } from "../badge/badge.js";
import { Meter } from "../meter/meter.js";
import { Swatch, TOKEN_KIND_ORDER, tokenKindColor, type TokenKind } from "../charts/core.js";
import { useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_USAGE_VARIANTS = {
  limit: {
    hard: { classes: "", description: "Requests over the limit are refused" },
    soft: { classes: "", description: "Over the limit only alerts" },
  },
} as const;
export const KAESO_USAGE_DEFAULT_VARIANTS = { limit: "hard" } as const;

const ORDER = TOKEN_KIND_ORDER;

function kindLabels(t: UiText): Record<TokenKind, string> {
  return {
    input: t("usageBreakdown.input", "Input"),
    output: t("usageBreakdown.output", "Output"),
    cacheRead: t("usageBreakdown.cacheRead", "Cache read"),
    cacheWrite: t("usageBreakdown.cacheWrite", "Cache write"),
    reasoning: t("usageBreakdown.reasoning", "Reasoning"),
  };
}

export const KAESO_USAGE_BREAKDOWN_VARIANTS = {} as const;
export const KAESO_USAGE_BREAKDOWN_DEFAULT_VARIANTS = {} as const;

export const UsageBreakdown = forwardRef<HTMLDivElement, { tokens: Partial<Record<TokenKind, number>>; className?: string | undefined }>(
  function UsageBreakdown({ tokens, className }, ref) {
    const t = useUiText();
    const label = kindLabels(t);
    const kinds = ORDER.filter((k) => (tokens[k] ?? 0) > 0);
    const total = kinds.reduce((a, k) => a + (tokens[k] ?? 0), 0) || 1;
    return (
      <div ref={ref} className={cn("flex flex-col gap-12", className)}>
        <div
          role="img"
          aria-label={t("usageBreakdown.tokens", "Tokens: {list}", {
            list: kinds
              .map((k) => t("usageBreakdown.kindCount", "{kind} {count}", { kind: label[k], count: formatNumber(tokens[k] ?? 0) }))
              .join(t("usageBreakdown.listSeparator", ", ")),
          })}
          className="flex h-8 w-full gap-2 overflow-hidden rounded-pill"
        >
          {kinds.map((k) => (
            <span
              key={k}
              className="h-full min-w-2 first:rounded-l-pill last:rounded-r-pill width-motion"
              style={{ width: `${((tokens[k] ?? 0) / total) * 100}%`, background: tokenKindColor(k) }}
            />
          ))}
        </div>
        <dl className="flex flex-wrap gap-x-24 gap-y-8">
          {ORDER.filter((k) => tokens[k] !== undefined).map((k) => (
            <div key={k} className="flex flex-col gap-2">
              <dt className="flex items-center gap-8 text-small text-fg-secondary">
                <Swatch color={tokenKindColor(k)} />
                {label[k]}
              </dt>
              <dd
                className="pl-18 text-body font-medium text-fg tabular"
                title={t("usageBreakdown.tokenCount", "{count} tokens", { count: formatNumber(tokens[k] ?? 0) })}
              >
                {formatTokens(tokens[k] ?? 0)}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    );
  },
);
UsageBreakdown.displayName = "UsageBreakdown";

export type LimitKind = "requests" | "tokens" | "spend";
const KIND_ICON: Record<LimitKind, IconData> = { requests: icons.requestLog, tokens: icons.stack, spend: icons.budget };

export interface LimitMeterProps {
  kind: LimitKind;
  scope: ReactNode;
  window: ReactNode;
  used: number;
  limit: number;
  valueLabel: ReactNode;
  hard?: boolean | undefined;
  alerts?: number[] | undefined;
  resets?: ReactNode | undefined;
  className?: string | undefined;
}

export const LimitMeter = forwardRef<HTMLDivElement, LimitMeterProps>(function LimitMeter(
  { kind, scope, window: win, used, limit, valueLabel, hard = true, alerts = [0.8], resets, className },
  ref,
) {
  const t = useUiText();
  return (
    <Meter
      ref={ref}
      className={className}
      value={limit > 0 ? used / limit : 0}
      thresholds={alerts}
      leading={<Icon icon={KIND_ICON[kind]} size="sm" className="text-fg-secondary" />}
      label={scope}
      trailing={<Badge tone={hard ? "neutral" : "outline"}>{hard ? t("limitMeter.hard", "Hard") : t("limitMeter.soft", "Soft")}</Badge>}
      caption={
        <span className="flex flex-wrap items-baseline justify-between gap-x-12 gap-y-2">
          <span>
            <span className="font-medium text-fg-secondary tabular">{valueLabel}</span> · {win}
          </span>
          {resets && <span>{resets}</span>}
        </span>
      }
      aria-label={t("limitMeter.label", "{kind, select, requests {requests limit} tokens {tokens limit} other {spend limit}}", { kind })}
    />
  );
});
LimitMeter.displayName = "LimitMeter";
