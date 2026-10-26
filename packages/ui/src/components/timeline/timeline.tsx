import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatDate } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";
import { Tooltip } from "../tooltip/tooltip.js";

export const KAESO_TIMELINE_VARIANTS = {
  tone: {
    neutral: { classes: "bg-line-strong forced-line", description: "Ordinary events (default)" },
    success: { classes: "bg-success data-mark", description: "Something completed" },
    warning: { classes: "bg-warning data-mark", description: "Something changed that needs attention" },
    danger: { classes: "bg-danger data-mark", description: "Something failed or was removed" },
    info: { classes: "bg-info data-mark", description: "Informational" },
  },
} as const;
export const KAESO_TIMELINE_DEFAULT_VARIANTS = { tone: "neutral" } as const;

export interface TimelineEvent {
  id: string;
  title: ReactNode;
  description?: ReactNode | undefined;
  time: ReactNode;
  datetime?: string | undefined;
  tone?: keyof typeof KAESO_TIMELINE_VARIANTS.tone | undefined;
  marker?: ReactNode | undefined;
}

export const Timeline = forwardRef<HTMLOListElement, { events: TimelineEvent[]; className?: string | undefined }>(function Timeline(
  { events, className },
  ref,
) {
  const t = useUiText();
  const toneWord = {
    neutral: "",
    success: t("timeline.success", "Success"),
    warning: t("timeline.warning", "Warning"),
    danger: t("timeline.danger", "Critical"),
    info: t("timeline.info", "Info"),
  };
  return (
    <ol ref={ref} className={cn("flex flex-col", className)}>
      {events.map((e, i) => (
        <li key={e.id} className="relative flex gap-16 pb-20 last:pb-0">
          {i < events.length - 1 && <span aria-hidden className="absolute top-20 bottom-0 left-12 w-1 bg-line forced-line" />}
          <span className="relative flex h-20 w-24 shrink-0 items-center justify-center">
            {e.marker ?? <span aria-hidden className={cn("h-14 w-3 rounded-pill", KAESO_TIMELINE_VARIANTS.tone[e.tone ?? "neutral"].classes)} />}
          </span>
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <div className="flex items-baseline justify-between gap-16">
              <span className="min-w-0 text-body text-fg">
                {e.tone && e.tone !== "neutral" && <span className="sr-only">{toneWord[e.tone]}: </span>}
                {e.title}
              </span>
              {e.datetime ? (
                <Tooltip content={formatDate(e.datetime, { time: true })}>
                  <time dateTime={e.datetime} className="shrink-0 text-small text-fg-secondary tabular">
                    {e.time}
                  </time>
                </Tooltip>
              ) : (
                <time className="shrink-0 text-small text-fg-secondary tabular">{e.time}</time>
              )}
            </div>
            {e.description && <div className="text-small text-fg-secondary">{e.description}</div>}
          </div>
        </li>
      ))}
    </ol>
  );
});
Timeline.displayName = "Timeline";
