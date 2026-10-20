import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { Badge } from "../badge/badge.js";
import { CopyButton } from "../copy-button/copy-button.js";
import { PROVIDER_MARKS } from "./provider-marks.js";
import { formatNumber } from "../../format/index.js";
import { useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_IDENTITY_VARIANTS = {
  provider: {
    sm: { classes: "size-16 rounded-cell text-micro", description: "16px, inside table cells" },
    md: { classes: "size-20 rounded-avatar text-micro", description: "20px (default)" },
    lg: { classes: "size-28 rounded-org text-caption", description: "28px" },
    xl: { classes: "size-40 rounded-org text-body", description: "40px: a provider's own page or card" },
  },
} as const;
export const KAESO_IDENTITY_DEFAULT_VARIANTS = { provider: "md" } as const;

const PROVIDER_LETTERS: Record<string, string> = {
  openai: "O",
  anthropic: "A",
  google: "G",
  mistral: "M",
  groq: "Gq",
  deepseek: "D",
  xai: "S",
  cohere: "C",
  bedrock: "B",
  azure: "Az",
  ollama: "Ol",
  togetherai: "T",
  openrouter: "Or",
};
const PROVIDER_NAMES: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  google: "Google",
  mistral: "Mistral",
  groq: "Groq",
  deepseek: "DeepSeek",
  xai: "SpaceX",
  cohere: "Cohere",
  bedrock: "Amazon Bedrock",
  azure: "Azure OpenAI",
  ollama: "Ollama",
  togetherai: "Together AI",
  openrouter: "OpenRouter",
};
export const providerName = (id: string) => PROVIDER_NAMES[id] ?? id;

const MARK_SIZE = { sm: "size-10", md: "size-12", lg: "size-16", xl: "size-24" } as const;

export const ProviderBadge = forwardRef<
  HTMLSpanElement,
  { provider: string; size?: keyof typeof KAESO_IDENTITY_VARIANTS.provider | undefined; withName?: boolean | undefined; className?: string | undefined }
>(function ProviderBadge({ provider, size = "md", withName, className }, ref) {
  const mark = PROVIDER_MARKS[provider];
  const tile = (
    <span
      aria-hidden={withName || undefined}
      {...(!withName ? { role: "img", "aria-label": providerName(provider) } : {})}
      className={cn(
        "inline-flex shrink-0 items-center justify-center bg-fill-1 font-semibold shadow-ring",
        mark ? mark.color : "text-fg-secondary",
        KAESO_IDENTITY_VARIANTS.provider[size].classes,
      )}
    >
      {mark ? (
        <svg viewBox="0 0 24 24" fill="currentColor" fillRule="evenodd" aria-hidden focusable="false" className={MARK_SIZE[size]}>
          <path d={mark.d} />
        </svg>
      ) : (
        (PROVIDER_LETTERS[provider] ?? provider.slice(0, 1).toUpperCase())
      )}
    </span>
  );
  if (!withName)
    return (
      <span ref={ref} className={cn("inline-flex", className)}>
        {tile}
      </span>
    );
  return (
    <span ref={ref} className={cn("inline-flex max-w-full min-w-0 items-center gap-8 text-body text-fg", className)}>
      {tile}
      <span className="min-w-0 truncate">{providerName(provider)}</span>
    </span>
  );
});
ProviderBadge.displayName = "ProviderBadge";

export type Capability = "tools" | "reasoning" | "vision" | "pdf" | "structuredOutput";
const CAP_ICON: Record<Capability, IconData> = {
  tools: icons.tools,
  reasoning: icons.reasoning,
  vision: icons.vision,
  pdf: icons.pdf,
  structuredOutput: icons.structuredOutput,
};
function capabilityLabels(t: UiText): Record<Capability, string> {
  return {
    tools: t("modelLabel.toolCalling", "Tool calling"),
    reasoning: t("modelLabel.reasoning", "Reasoning"),
    vision: t("modelLabel.imageInput", "Image input"),
    pdf: t("modelLabel.pdfInput", "PDF input"),
    structuredOutput: t("modelLabel.structuredOutput", "Structured output"),
  };
}

const SHORT_ID = 14;

export interface ModelLabelProps {
  provider: string;
  model: string;
  name?: string | undefined;
  capabilities?: Capability[] | undefined;
  className?: string | undefined;
}

export const ModelLabel = forwardRef<HTMLSpanElement, ModelLabelProps>(function ModelLabel({ provider, model, name, capabilities = [], className }, ref) {
  return (
    <span ref={ref} className={cn("inline-flex max-w-full min-w-0 items-center gap-8", className)}>
      <ProviderBadge provider={provider} />
      {name ? (
        <>
          <span className="min-w-0 truncate text-body text-fg">{name}</span>
          <span className="flex h-20 min-w-0 flex-1 basis-0 flex-wrap content-start items-center overflow-hidden">
            <span aria-hidden className="h-20 w-0" />
            <span className={cn("flex-1 truncate mono text-small text-fg-secondary", model.length <= SHORT_ID ? "min-w-max" : "min-w-96")}>{model}</span>
          </span>
        </>
      ) : (
        <span className="min-w-0 truncate mono text-small text-fg">{model}</span>
      )}
      {capabilities.length > 0 && <CapabilityIcons capabilities={capabilities} className="hidden sm:flex" />}
    </span>
  );
});
ModelLabel.displayName = "ModelLabel";

export const CapabilityIcons = forwardRef<HTMLSpanElement, { capabilities: Capability[]; className?: string | undefined }>(function CapabilityIcons(
  { capabilities, className },
  ref,
) {
  const t = useUiText();
  const capLabel = capabilityLabels(t);
  if (capabilities.length === 0) return null;
  return (
    <span
      ref={ref}
      role="img"
      className={cn("flex shrink-0 items-center gap-2", className)}
      aria-label={t("modelLabel.capabilities", "Capabilities: {list}", {
        list: capabilities.map((c) => capLabel[c]).join(t("modelLabel.listSeparator", ", ")),
      })}
    >
      {capabilities.map((c) => (
        <Tooltip key={c} content={capLabel[c]}>
          <span className="inline-flex size-20 items-center justify-center rounded-pill text-fg-tertiary hover:text-fg">
            <Icon icon={CAP_ICON[c]} size="xs" />
          </span>
        </Tooltip>
      ))}
    </span>
  );
});
CapabilityIcons.displayName = "CapabilityIcons";

export type KeyState = "active" | "disabled" | "expired" | "archived";
const KEY_TONE: Record<KeyState, "success" | "neutral" | "warning"> = { active: "success", disabled: "neutral", expired: "warning", archived: "neutral" };

export interface KeyDisplayProps {
  redacted: string;
  state?: KeyState | undefined;
  access?: "inference" | "platform" | "both" | undefined;
  copy?: boolean | undefined;
  className?: string | undefined;
}

function accessLabels(t: UiText) {
  return {
    inference: t("keyDisplay.inference", "Inference"),
    platform: t("keyDisplay.platform", "Platform"),
    both: t("keyDisplay.inferenceAndPlatform", "Inference + platform"),
  };
}

function stateLabels(t: UiText): Record<KeyState, string> {
  return {
    active: t("keyDisplay.active", "Active"),
    disabled: t("keyDisplay.disabled", "Disabled"),
    expired: t("keyDisplay.expired", "Expired"),
    archived: t("keyDisplay.archived", "Archived"),
  };
}

export const KeyDisplay = forwardRef<HTMLSpanElement, KeyDisplayProps>(function KeyDisplay(
  { redacted, state = "active", access, copy = true, className },
  ref,
) {
  const t = useUiText();
  return (
    <span ref={ref} className={cn("group/key inline-flex max-w-full min-w-0 items-center gap-8", className)}>
      <span className={cn("min-w-0 truncate mono text-small", state === "active" ? "text-fg" : "text-fg-secondary")}>{redacted}</span>
      {state !== "active" && <Badge tone={KEY_TONE[state] === "warning" ? "warning" : "neutral"}>{stateLabels(t)[state]}</Badge>}
      {access && <Badge tone={access === "inference" ? "neutral" : "outline"}>{accessLabels(t)[access]}</Badge>}
      {copy && (
        <span className="opacity-0 transition-control group-hover/key:opacity-100 coarse:opacity-100 focus-within:opacity-100">
          <CopyButton value={redacted} size="xs" label={t("keyDisplay.copyRedactedKey", "Copy redacted key")} />
        </span>
      )}
    </span>
  );
});
KeyDisplay.displayName = "KeyDisplay";

const STATUS_CLASS = (code: number) => (code >= 500 ? "danger" : code >= 400 ? "warning" : "neutral") as "danger" | "warning" | "neutral";
function statusTexts(t: UiText): Record<number, string> {
  return {
    200: t("statusCode.ok", "OK"),
    201: t("statusCode.created", "Created"),
    204: t("statusCode.noContent", "No content"),
    400: t("statusCode.badRequest", "Bad request"),
    401: t("statusCode.unauthorized", "Unauthorized"),
    403: t("statusCode.forbidden", "Forbidden"),
    404: t("statusCode.notFound", "Not found"),
    408: t("statusCode.timeout", "Timeout"),
    409: t("statusCode.conflict", "Conflict"),
    413: t("statusCode.tooLarge", "Too large"),
    422: t("statusCode.unprocessable", "Unprocessable"),
    429: t("statusCode.rateLimited", "Rate limited"),
    499: t("statusCode.clientClosed", "Client closed"),
    500: t("statusCode.serverError", "Server error"),
    502: t("statusCode.badGateway", "Bad gateway"),
    503: t("statusCode.unavailable", "Unavailable"),
    504: t("statusCode.gatewayTimeout", "Gateway timeout"),
    529: t("statusCode.overloaded", "Overloaded"),
  };
}

export const StatusCode = forwardRef<HTMLSpanElement, { code: number; showText?: boolean | undefined; className?: string | undefined }>(function StatusCode(
  { code, showText, className },
  ref,
) {
  const t = useUiText();
  const text = statusTexts(t)[code];
  const tone = STATUS_CLASS(code);
  const color = { danger: "text-danger-text", warning: "text-warning-text", neutral: code >= 200 && code < 300 ? "text-fg" : "text-fg-secondary" }[tone];
  const el = (
    <span ref={ref} tabIndex={showText ? undefined : 0} className={cn("inline-flex items-center gap-6 outline-none", className)}>
      <span className={cn("mono text-small font-medium tabular", color)}>{code}</span>
      {showText && <span className="text-small text-fg-secondary">{text ?? ""}</span>}
      <span className="sr-only">{text ?? ""}</span>
    </span>
  );
  return showText || !text ? el : <Tooltip content={t("statusCode.tooltip", "{code} {text}", { code: String(code), text })}>{el}</Tooltip>;
});
StatusCode.displayName = "StatusCode";

export const RequestTimeline = forwardRef<
  HTMLDivElement,
  { ttfbMs: number; totalMs: number; width?: 160 | 240 | "full" | undefined; className?: string | undefined }
>(function RequestTimeline({ ttfbMs, totalMs, width = 240, className }, ref) {
  const t = useUiText();
  const ratio = totalMs > 0 ? Math.min(1, ttfbMs / totalMs) : 0;
  const fmt = (ms: number) =>
    ms < 1000 ? `${Math.round(ms)} ms` : `${formatNumber(ms / 1000, { minimumFractionDigits: 1, maximumFractionDigits: 1, useGrouping: false })} s`;
  return (
    <div
      ref={ref}
      className={cn("flex flex-col gap-6", width === "full" ? "w-full" : width === 240 ? "w-240" : "w-160", className)}
      role="img"
      aria-label={t("requestTimeline.label", "Time to first byte {ttfb} of {total} total", { ttfb: fmt(ttfbMs), total: fmt(totalMs) })}
    >
      <div className="flex h-6 w-full gap-2 overflow-hidden rounded-pill">
        <span className="h-full rounded-l-pill bg-viz-1" style={{ width: `${ratio * 100}%` }} />
        <span className="h-full flex-1 rounded-r-pill bg-fill-3" />
      </div>
      <div className="flex justify-between gap-8 text-caption font-medium text-fg-tertiary">
        <span className="flex min-w-0 flex-col">
          <span className="whitespace-nowrap text-fg-secondary tabular">{fmt(ttfbMs)}</span>
          <span className="wrap-break-word">{t("requestTimeline.firstByte", "First byte")}</span>
        </span>
        <span className="flex min-w-0 flex-col items-end text-right">
          <span className="whitespace-nowrap text-fg-secondary tabular">{fmt(totalMs)}</span>
          <span className="wrap-break-word">{t("requestTimeline.totalTime", "Total")}</span>
        </span>
      </div>
    </div>
  );
});
RequestTimeline.displayName = "RequestTimeline";

export interface ProviderCardProps {
  provider: string;
  connected: boolean;
  status: ReactNode;
  keyLabel?: ReactNode | undefined;
  detail?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  className?: string | undefined;
}

export const ProviderCard = forwardRef<HTMLDivElement, ProviderCardProps>(function ProviderCard(
  { provider, connected, status, keyLabel, detail, actions, className },
  ref,
) {
  const t = useUiText();
  return (
    <section ref={ref} aria-label={providerName(provider)} className={cn("flex min-w-0 flex-col gap-16 rounded-group bg-raised p-20 shadow-xs", className)}>
      <div className="flex min-w-0 items-center gap-12">
        <ProviderBadge provider={provider} size="xl" />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h3 className="truncate text-body-lg font-semibold text-fg">{providerName(provider)}</h3>
          <span className="text-small text-fg-secondary text-pretty">{status}</span>
        </div>
        <Badge tone={connected ? "success" : "neutral"}>
          {connected ? t("providerCard.connected", "Connected") : t("providerCard.notConnected", "Not connected")}
        </Badge>
      </div>
      {(keyLabel || detail) && (
        <div className="flex min-w-0 flex-col gap-2 rounded-well bg-fill-1 px-12 py-10">
          {keyLabel && <span className="truncate text-body text-fg">{keyLabel}</span>}
          {detail && <span className="truncate text-small text-fg-tertiary">{detail}</span>}
        </div>
      )}
      {actions && <div className="mt-auto flex flex-wrap items-center gap-8">{actions}</div>}
    </section>
  );
});
ProviderCard.displayName = "ProviderCard";
