import { forwardRef, useId, useRef, useState, type ReactNode } from "react";
import { Collapsible as BaseCollapsible } from "@base-ui/react/collapsible";
import { cn } from "../../utils/cn";
import { icons } from "../../icons/vocabulary";
import { formatDuration } from "../../format";
import { Icon } from "../icon/icon";
import { Button } from "../button/button";
import { Markdown } from "../markdown/markdown";
import { useTruncated } from "../../hooks/use-truncated";
import { useUiText } from "../../i18n/context";

export const KAESO_RESPONSE_VIEW_VARIANTS = {
  status: {
    pending: { classes: "", description: "Sent, nothing back yet: an empty line where the text will start, nothing drawn" },
    streaming: { classes: "", description: "Arriving: the text grows, earlier lines never re-wrap; Thinking is live" },
    done: { classes: "", description: "Complete (default)" },
    stopped: { classes: "", description: "Stopped by the person: the text so far and “Stopped”" },
    error: { classes: "", description: "Failed: “No response” and Retry. The error itself is a toast" },
  },
} as const;
export const KAESO_RESPONSE_VIEW_DEFAULT_VARIANTS = { status: "done" } as const;

export type ResponseStatus = keyof typeof KAESO_RESPONSE_VIEW_VARIANTS.status;

export interface ResponseViewProps {
  text?: string | undefined;
  reasoning?: string | undefined;
  status?: ResponseStatus | undefined;
  reasoningDuration?: number | undefined;
  reasoningOpen?: boolean | undefined;
  defaultReasoningOpen?: boolean | undefined;
  onReasoningOpenChange?: ((open: boolean) => void) | undefined;
  onRetry?: (() => void) | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

function lastLine(text: string) {
  const lines = text.trim().split("\n");
  for (let i = lines.length - 1; i >= 0; i--) {
    const l = (lines[i] ?? "").replace(/[*_`#>]+/g, "").trim();
    if (l) return l;
  }
  return "";
}

function Thinking({
  text,
  live,
  duration,
  open,
  defaultOpen,
  onOpenChange,
}: {
  text: string;
  live: boolean;
  duration: number | undefined;
  open: boolean | undefined;
  defaultOpen: boolean | undefined;
  onOpenChange: ((open: boolean) => void) | undefined;
}) {
  const t = useUiText();
  const preview = live ? lastLine(text) : "";
  const title = live
    ? t("responseView.thinking", "Thinking")
    : duration !== undefined
      ? t("responseView.thoughtFor", "Thought for {duration}", { duration: formatDuration(duration) })
      : t("responseView.thinking", "Thinking");
  return (
    <BaseCollapsible.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      className="flex min-w-0 flex-col"
    >
      <BaseCollapsible.Trigger className="group/think -mx-8 flex h-32 min-w-0 cursor-pointer items-center gap-6 rounded-pill px-8 text-left text-small text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring-inset">
        <span className={cn("shrink-0 font-medium", live && "animate-pulse-quiet reduced:animate-none")}>{title}</span>
        <Icon
          icon={icons.chevronRight}
          size="xs"
          className="shrink-0 text-fg-tertiary transition-transform duration-180 ease-overlay group-data-panel-open/think:rotate-90"
        />
        <span aria-hidden className="min-w-0 flex-1 truncate text-fg-tertiary group-data-panel-open/think:invisible">
          {preview}
        </span>
      </BaseCollapsible.Trigger>
      <BaseCollapsible.Panel className="collapse-motion">
        <div className="flex gap-12 pt-4 pb-8">
          <span aria-hidden className="w-1 shrink-0 rounded-pill bg-line" />
          <Markdown text={text} size="small" tone="secondary" streaming={live} collapseAfter={0} wrapCode className="min-w-0 flex-1" />
        </div>
      </BaseCollapsible.Panel>
    </BaseCollapsible.Root>
  );
}

export const ResponseView = forwardRef<HTMLElement, ResponseViewProps>(function ResponseView(
  {
    text = "",
    reasoning,
    status = "done",
    reasoningDuration,
    reasoningOpen,
    defaultReasoningOpen,
    onReasoningOpenChange,
    onRetry,
    footer,
    "aria-label": ariaLabelProp,
    className,
  },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("responseView.response", "Response");
  const announce = useId();
  const active = status === "pending" || status === "streaming";
  const hasText = text.trim() !== "";
  const hasReasoning = !!reasoning && reasoning.trim() !== "";
  const thinking = active && hasReasoning && !hasText;
  return (
    <article
      ref={ref}
      aria-label={ariaLabel}
      aria-busy={active || undefined}
      aria-describedby={announce}
      className={cn("flex min-w-0 flex-col gap-8", className)}
    >
      {hasReasoning && (
        <Thinking
          text={reasoning}
          live={thinking}
          duration={reasoningDuration}
          open={reasoningOpen}
          defaultOpen={defaultReasoningOpen}
          onOpenChange={onReasoningOpenChange}
        />
      )}
      {hasText ? (
        <Markdown text={text} size="lg" streaming={active} collapseAfter={0} wrapCode />
      ) : active && !hasReasoning ? (
        <div aria-hidden className="h-22" />
      ) : null}
      {status === "stopped" && (
        <div className="flex min-h-24 flex-wrap items-center gap-x-12 gap-y-4">
          <span className="text-small text-fg-tertiary">
            {hasText ? t("responseView.stopped", "Stopped") : t("responseView.stoppedBeforeResponse", "Stopped before a response")}
          </span>
          {onRetry && (
            <Button size="xs" variant="ghost" onClick={onRetry}>
              {t("responseView.retry", "Retry")}
            </Button>
          )}
        </div>
      )}
      {status === "error" && (
        <div className="flex min-h-24 flex-wrap items-center gap-x-12 gap-y-4">
          <span className="text-small text-fg-secondary">
            {hasText ? t("responseView.stoppedEarly", "The response stopped early") : t("responseView.noResponse", "No response")}
          </span>
          {onRetry && (
            <Button size="xs" variant="secondary" onClick={onRetry}>
              {t("responseView.retry", "Retry")}
            </Button>
          )}
        </div>
      )}
      {status === "done" && !hasText && <span className="text-small text-fg-tertiary">{t("responseView.noText", "The model returned no text")}</span>}
      {footer}
      <span id={announce} className="sr-only">
        {status === "pending"
          ? t("responseView.waiting", "Waiting for the response")
          : status === "streaming"
            ? t("responseView.responding", "Responding")
            : ""}
      </span>
    </article>
  );
});
ResponseView.displayName = "ResponseView";

export interface UserMessageProps {
  children: ReactNode;
  maxLines?: 0 | 2 | 3 | 4 | 6 | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const CLAMP = { 2: "line-clamp-2", 3: "line-clamp-3", 4: "line-clamp-4", 6: "line-clamp-6" } as const;

export const UserMessage = forwardRef<HTMLDivElement, UserMessageProps>(function UserMessage(
  { children, maxLines = 4, "aria-label": ariaLabelProp, className },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("userMessage.prompt", "Prompt");
  const showAll = t("userMessage.showAll", "Show all");
  const showLess = t("userMessage.showLess", "Show less");
  const [all, setAll] = useState(false);
  const text = useRef<HTMLParagraphElement | null>(null);
  const clamped = useTruncated(text, maxLines > 0 && !all);
  return (
    <div ref={ref} role="group" aria-label={ariaLabel} className={cn("flex min-w-0 flex-col items-start gap-2", className)}>
      <p
        ref={text}
        className={cn(
          "max-w-prose min-w-0 text-body font-medium whitespace-pre-wrap wrap-break-word text-fg-secondary",
          maxLines !== 0 && !all && CLAMP[maxLines],
        )}
      >
        {children}
      </p>
      {(clamped || all) && (
        <Button size="xs" variant="link" reserveLabels={[showAll, showLess]} onClick={() => setAll((a) => !a)} className="text-fg-tertiary">
          {all ? showLess : showAll}
        </Button>
      )}
    </div>
  );
});
UserMessage.displayName = "UserMessage";

export interface ThreadProps {
  children: ReactNode;
  className?: string | undefined;
}

export const Thread = forwardRef<HTMLDivElement, ThreadProps>(function Thread({ children, className }, ref) {
  return (
    <div ref={ref} className={cn("flex min-w-0 flex-col gap-32", className)}>
      {children}
    </div>
  );
});
Thread.displayName = "Thread";
