import { forwardRef, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { formatNumber } from "../../format/index.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";
import { Button } from "../button/button.js";
import { Markdown } from "../markdown/markdown.js";

export const KAESO_MEDIA_RESPONSE_VARIANTS = {
  kind: {
    image: { classes: "", description: "Images in a grid of squares, two across, each with Download (default)" },
    audio: { classes: "", description: "One audio player a row tall, with Download" },
    video: { classes: "", description: "A 16:9 video player; while the job runs, its progress and Cancel" },
  },
} as const;
export const KAESO_MEDIA_RESPONSE_DEFAULT_VARIANTS = { kind: "image" } as const;

export type MediaKind = keyof typeof KAESO_MEDIA_RESPONSE_VARIANTS.kind;
export type MediaStatus = "pending" | "running" | "done" | "stopped" | "error";

/** One file a model made: where to play or show it from, the name it downloads under, and what people read. */
export interface MediaItem {
  src: string;
  /** The file name it downloads under, such as `image-1.png`. */
  name: string;
  /** What screen readers and Download say, in the reader's language, such as "Image 1"; the file name when absent. */
  label?: string | undefined;
  /** What it shows, for an image: read by screen readers. */
  alt?: string | undefined;
}

export interface MediaResponseProps {
  kind?: MediaKind | undefined;
  status?: MediaStatus | undefined;
  items?: readonly MediaItem[] | undefined;
  expected?: number | undefined;
  progress?: number | null | undefined;
  onCancel?: (() => void) | undefined;
  error?: string | undefined;
  onRetry?: (() => void) | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const FRAME = "relative min-w-0 overflow-hidden rounded-frame bg-fill-1 image-outline";
const WAITING = "animate-pulse-quiet reduced:animate-none";
const VIDEO_FRAME = { aspectRatio: "16 / 9" } as const;

function Download({ item, label }: { item: MediaItem; label: string }) {
  return <Button size="sm" variant="secondary" icon={icons.download} aria-label={label} href={item.src} download={item.name} />;
}

function Images({ items, expected, waiting, t }: { items: readonly MediaItem[]; expected: number; waiting: boolean; t: ReturnType<typeof useUiText> }) {
  const count = waiting ? Math.max(1, expected) : items.length;
  return (
    <ul className={cn("grid gap-8", count > 1 ? "grid-cols-2" : "max-w-480 grid-cols-1")}>
      {Array.from({ length: count }, (_, i) => {
        const item = waiting ? undefined : items[i];
        return (
          <li key={item?.src ?? i} className={cn(FRAME, "aspect-square", !item && WAITING)}>
            {item && (
              <>
                <img src={item.src} alt={item.alt ?? item.label ?? item.name} decoding="async" className="size-full object-contain" />
                <span className="absolute right-8 bottom-8">
                  <Download item={item} label={t("mediaResponse.download", "Download {name}", { name: item.label ?? item.name })} />
                </span>
              </>
            )}
          </li>
        );
      })}
    </ul>
  );
}

function useElapsed(running: boolean) {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (!running) return;
    const started = Date.now();
    setSeconds(0);
    const id = setInterval(() => setSeconds(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => clearInterval(id);
  }, [running]);
  return seconds;
}

export const MediaResponse = forwardRef<HTMLElement, MediaResponseProps>(function MediaResponse(
  { kind = "image", status = "done", items = [], expected = 1, progress, onCancel, error, onRetry, footer, "aria-label": ariaLabelProp, className },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("responseView.response", "Response");
  const waiting = status === "pending" || status === "running";
  const said = useId();
  const elapsed = useElapsed(waiting && kind === "video");
  const before = useRef(status);
  const [announced, setAnnounced] = useState("");
  useEffect(() => {
    const was = before.current;
    before.current = status;
    if (waiting) setAnnounced("");
    else if ((was === "pending" || was === "running") && status === "done") setAnnounced(t("responseView.endedDone", "{label} finished", { label: ariaLabel }));
  }, [status, waiting, ariaLabel, t]);
  const percent = typeof progress === "number" && Number.isFinite(progress) ? Math.max(0, Math.min(100, progress)) : null;
  const making =
    kind === "image"
      ? t("mediaResponse.makingImages", "Making the image")
      : kind === "audio"
        ? t("mediaResponse.makingAudio", "Making the audio")
        : status === "pending" || percent === null || percent === 0
          ? t("mediaResponse.queued", "Waiting for the provider to start the video")
          : t("mediaResponse.makingVideo", "Making the video, {percent}% done", { percent: formatNumber(Math.round(percent)) });
  const hasFooter = footer !== undefined && footer !== null;
  const retryLabel = t("responseView.retry", "Retry");
  const saveVideo = status === "done" && kind === "video" && items[0];
  const tools =
    status === "done" && (onRetry || saveVideo) ? (
      <span
        data-reply-actions=""
        className={cn(
          "flex items-center gap-x-4 transition-arrive reduced:transition-none starting:opacity-0",
          saveVideo && onRetry ? "starting:-ml-72" : "starting:-ml-36",
        )}
      >
        {saveVideo && (
          <Button
            size="sm"
            variant="quiet"
            icon={icons.download}
            aria-label={t("mediaResponse.download", "Download {name}", { name: saveVideo.label ?? saveVideo.name })}
            href={saveVideo.src}
            download={saveVideo.name}
          />
        )}
        {onRetry && <Button size="sm" variant="quiet" icon={icons.retry} aria-label={retryLabel} onClick={onRetry} />}
      </span>
    ) : null;
  const item = items[0];
  return (
    <article
      ref={ref}
      aria-label={ariaLabel}
      aria-busy={waiting || undefined}
      {...(waiting ? { "aria-describedby": said } : {})}
      className={cn("flex min-w-0 flex-col gap-8", className)}
    >
      {kind === "image" && (waiting || items.length > 0) && <Images items={items} expected={expected} waiting={waiting} t={t} />}
      {kind === "audio" &&
        (waiting ? (
          <div aria-hidden className={cn("h-56 rounded-pill bg-fill-1", WAITING)} />
        ) : item ? (
          <div className="flex h-56 min-w-0 items-center gap-8">
            <audio controls preload="metadata" src={item.src} aria-label={item.label ?? item.name} className="h-40 min-w-0 flex-1" />
            <Download item={item} label={t("mediaResponse.download", "Download {name}", { name: item.label ?? item.name })} />
          </div>
        ) : null)}
      {kind === "video" &&
        (waiting || item ? (
          <div className={cn(FRAME, waiting && "flex flex-col items-center justify-center gap-12 px-24")} style={VIDEO_FRAME}>
            {waiting ? (
              <>
                <div
                  role="progressbar"
                  aria-label={making}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  {...(percent !== null ? { "aria-valuenow": Math.round(percent) } : {})}
                  className="h-6 w-full max-w-240 overflow-hidden rounded-pill bg-fill-2"
                >
                  <div
                    className={cn("h-full rounded-pill bg-fg-secondary", percent === null && WAITING)}
                    style={{ width: `${percent === null ? 100 : Math.max(4, percent)}%` }}
                  />
                </div>
                {onCancel && (
                  <Button size="sm" variant="secondary" onClick={onCancel}>
                    {t("mediaResponse.cancel", "Cancel")}
                  </Button>
                )}
              </>
            ) : (
              item && <video controls playsInline preload="metadata" src={item.src} aria-label={item.label ?? item.name} className="size-full object-contain" />
            )}
          </div>
        ) : null)}
      {waiting && (
        <p id={said} className="flex min-h-22 items-center gap-8 px-12 text-small text-fg-secondary tabular">
          <span className="min-w-0 truncate">{making}</span>
          {kind === "video" && elapsed > 0 && <span className="text-fg-tertiary">{t("format.seconds", "{s} s", { s: formatNumber(elapsed) })}</span>}
        </p>
      )}
      {status === "done" && items.length === 0 && (
        <span className="px-12 text-small text-fg-tertiary">{t("mediaResponse.nothing", "The model returned nothing to show")}</span>
      )}
      {status === "stopped" && (
        <div className="flex min-h-32 flex-wrap items-center gap-x-12 gap-y-4 px-12">
          <span className="text-small text-fg-secondary">{t("mediaResponse.stopped", "Stopped before it finished")}</span>
          {onRetry && (
            <Button size="sm" variant="secondary" onClick={onRetry}>
              {t("responseView.retry", "Retry")}
            </Button>
          )}
        </div>
      )}
      {status === "error" && (
        <div className="flex flex-col items-start gap-8 px-12">
          <div role="alert" className="flex min-w-0 flex-col gap-4">
            <span className="text-small font-medium text-fg">{t("responseView.noResponse", "No response")}</span>
            {error && <Markdown text={error} size="small" tone="secondary" className="max-w-prose min-w-0" />}
          </div>
          {onRetry && (
            <Button size="sm" variant="secondary" onClick={onRetry}>
              {t("responseView.retry", "Retry")}
            </Button>
          )}
        </div>
      )}
      {(hasFooter || tools) && (
        <div className="flex min-h-32 min-w-0 flex-wrap items-center gap-x-4 gap-y-4">
          {tools}
          {hasFooter && <div className={cn("min-w-0", tools ? "px-8" : "px-12")}>{footer}</div>}
        </div>
      )}
      <span role="status" className="sr-only">
        {announced}
      </span>
    </article>
  );
});
MediaResponse.displayName = "MediaResponse";
