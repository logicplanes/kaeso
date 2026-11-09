import { forwardRef, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Button } from "../button/button.js";
import { CopyButton } from "../copy-button/copy-button.js";
import { icons } from "../../icons/vocabulary.js";
import { Markdown } from "../markdown/markdown.js";
import { Reasoning } from "./reasoning.js";
import { useTruncated } from "../../hooks/use-truncated.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { Attachments, type Attachment } from "./attachments.js";

export const KAESO_RESPONSE_VIEW_VARIANTS = {
  status: {
    pending: { classes: "", description: "Sent, nothing back yet: an empty line where the text will start, nothing drawn" },
    streaming: { classes: "", description: "Arriving: the text grows, earlier lines never re-wrap; Thinking is live" },
    done: { classes: "", description: "Complete (default)" },
    stopped: { classes: "", description: "Stopped by the person: the text so far and “Stopped”" },
    incomplete: { classes: "", description: "Cut off by the model: the text so far and why, such as “Stopped at the output limit”" },
    error: { classes: "", description: "Failed: the error in place of the response, and Retry" },
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
  incompleteReason?: string | undefined;
  error?: string | undefined;
  onRetry?: (() => void) | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

function incompleteText(t: UiText, reason: string | undefined): string {
  if (reason === "max_output_tokens") return t("responseView.outputLimit", "Stopped at the output limit");
  if (reason === "content_filter") return t("responseView.contentFilter", "Stopped by the provider's content filter");
  return t("responseView.incomplete", "The model ended the response early");
}

function useEndAnnouncement(status: ResponseStatus, ended: string) {
  const [said, setSaid] = useState("");
  const before = useRef(status);
  useEffect(() => {
    const was = before.current;
    before.current = status;
    if (status === "pending" || status === "streaming") setSaid("");
    else if ((was === "pending" || was === "streaming") && status !== "error") setSaid(ended);
  }, [status, ended]);
  return said;
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
    incompleteReason,
    error,
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
  const ended =
    status === "stopped"
      ? t("responseView.endedStopped", "{label} stopped", { label: ariaLabel })
      : status === "incomplete"
        ? t("responseView.endedIncomplete", "{label}: {reason}", { label: ariaLabel, reason: incompleteText(t, incompleteReason) })
        : t("responseView.endedDone", "{label} finished", { label: ariaLabel });
  const said = useEndAnnouncement(status, ended);
  const retryLabel = t("responseView.retry", "Retry");
  const retry = onRetry ? (
    <Button size="sm" variant="secondary" onClick={onRetry}>
      {retryLabel}
    </Button>
  ) : null;
  const copyLabel = t("responseView.copy", "Copy response");
  const hasFooter = footer !== undefined && footer !== null;
  const tools =
    status === "done" && (hasText || onRetry) ? (
      <>
        {hasText && <CopyButton value={text} size="sm" variant="quiet" label={copyLabel} />}
        {onRetry && <Button size="sm" variant="quiet" icon={icons.retry} aria-label={retryLabel} onClick={onRetry} />}
      </>
    ) : active && hasFooter ? (
      <span aria-hidden data-tools-reserved="" className="invisible flex items-center gap-x-4">
        <Button size="sm" variant="quiet" icon={icons.copy} aria-label={copyLabel} tabIndex={-1} />
        <Button size="sm" variant="quiet" icon={icons.retry} aria-label={retryLabel} tabIndex={-1} />
      </span>
    ) : null;
  return (
    <article
      ref={ref}
      aria-label={ariaLabel}
      aria-busy={active || undefined}
      aria-describedby={announce}
      className={cn("flex min-w-0 flex-col gap-8", className)}
    >
      {hasReasoning && (
        <Reasoning
          text={reasoning}
          live={thinking}
          duration={reasoningDuration}
          open={reasoningOpen}
          defaultOpen={defaultReasoningOpen}
          onOpenChange={onReasoningOpenChange}
        />
      )}
      {hasText ? <Markdown text={text} size="lg" streaming={active} collapseAfter={0} wrapCode className="markdown-text-inset" /> : null}
      {(status === "stopped" || status === "incomplete") && (
        <div className="flex min-h-32 flex-wrap items-center gap-x-12 gap-y-4 px-12">
          <span className="text-small text-fg-secondary">
            {status === "incomplete"
              ? incompleteText(t, incompleteReason)
              : hasText
                ? t("responseView.stopped", "Stopped")
                : t("responseView.stoppedBeforeResponse", "Stopped before a response")}
          </span>
          {retry}
        </div>
      )}
      {status === "error" && (
        <div className="flex flex-col items-start gap-8 px-12">
          <div role="alert" className="flex min-w-0 flex-col gap-4">
            <span className="text-small font-medium text-fg">
              {hasText ? t("responseView.failedPartway", "The response failed partway") : t("responseView.noResponse", "No response")}
            </span>
            {error && <Markdown text={error} size="small" tone="secondary" className="max-w-prose min-w-0" />}
          </div>
          {retry}
        </div>
      )}
      {status === "done" && !hasText && <span className="px-12 text-small text-fg-tertiary">{t("responseView.noText", "The model returned no text")}</span>}
      {(tools || hasFooter) && (
        <div data-reply-tools="" className={cn("flex min-h-32 min-w-0 flex-wrap items-center gap-x-4 gap-y-4", tools && "pl-4")}>
          {tools}
          {hasFooter && <div className={cn("min-w-0", tools ? "px-8" : "px-12")}>{footer}</div>}
        </div>
      )}
      <span id={announce} className="sr-only">
        {status === "pending"
          ? t("responseView.waiting", "Waiting for the response")
          : status === "streaming"
            ? t("responseView.responding", "Responding")
            : ""}
      </span>
      <span role="status" className="sr-only">
        {said}
      </span>
    </article>
  );
});
ResponseView.displayName = "ResponseView";

export interface UserMessageProps {
  children: ReactNode;
  attachments?: readonly Attachment[] | undefined;
  maxLines?: 0 | 2 | 3 | 4 | 6 | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const CLAMP = { 2: "line-clamp-2", 3: "line-clamp-3", 4: "line-clamp-4", 6: "line-clamp-6" } as const;

export const UserMessage = forwardRef<HTMLDivElement, UserMessageProps>(function UserMessage(
  { children, attachments = [], maxLines = 4, "aria-label": ariaLabelProp, className },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("userMessage.prompt", "Prompt");
  const showAll = t("userMessage.showAll", "Show all");
  const showLess = t("userMessage.showLess", "Show less");
  const [all, setAll] = useState(false);
  const body = useId();
  const text = useRef<HTMLParagraphElement | null>(null);
  const clamped = useTruncated(text, maxLines > 0 && !all);
  const hasText = children !== "" && children !== null && children !== undefined && children !== false;
  return (
    <div ref={ref} role="group" aria-label={ariaLabel} className={cn("flex min-w-0 flex-col items-end gap-2", className)}>
      <Attachments items={attachments} align="end" className={hasText ? "mb-6" : undefined} />
      {hasText && (
        <p
          id={body}
          ref={text}
          className={cn(
            "max-w-prose min-w-0 rounded-frame bg-fill-1 px-16 py-10 text-body-lg whitespace-pre-wrap wrap-break-word text-fg",
            maxLines !== 0 && !all && CLAMP[maxLines],
          )}
        >
          {children}
        </p>
      )}
      {(clamped || all) && (
        <Button size="sm" variant="quiet" aria-expanded={all} aria-controls={body} reserveLabels={[showAll, showLess]} onClick={() => setAll((a) => !a)}>
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
