import { forwardRef, useState } from "react";
import { useUiText } from "../../i18n/context.js";
import { cn } from "../../utils/cn.js";
import { Thumbnail } from "./thumbnail.js";

export const KAESO_MEDIA_VIEW_VARIANTS = {
  kind: {
    image: { classes: "", description: "A thumbnail that opens to the whole image in place, and closes again (default)" },
    audio: { classes: "w-full max-w-400", description: "The browser's audio player" },
    video: { classes: "w-full max-w-560 rounded-well bg-fill-1", description: "The browser's video player on a well" },
    file: { classes: "", description: "Nothing to play: the name, the detail and the download alone" },
  },
} as const;
export const KAESO_MEDIA_VIEW_DEFAULT_VARIANTS = { kind: "image" } as const;

export interface MediaViewProps {
  src: string;
  label: string;
  kind?: keyof typeof KAESO_MEDIA_VIEW_VARIANTS.kind | undefined;
  detail?: string | undefined;
  filename?: string | undefined;
  download?: string | undefined;
  className?: string | undefined;
}

export const MediaView = forwardRef<HTMLElement, MediaViewProps>(function MediaView(
  { src, label, kind = "image", detail, filename, download, className },
  ref,
) {
  const t = useUiText();
  const [open, setOpen] = useState(false);
  const classes = KAESO_MEDIA_VIEW_VARIANTS.kind[kind].classes;
  const link = (
    <a
      href={download ?? src}
      download={filename ?? ""}
      className="relative -mx-2 tap-target rounded-code px-2 font-medium text-fg no-underline outline-offset-0 transition-control focus-ring hover:underline hover:decoration-fg-tertiary hover:underline-offset-3"
    >
      {t("mediaView.download", "Download")}
    </a>
  );
  return (
    <figure ref={ref} className={cn("flex min-w-0 flex-col items-start gap-8", className)}>
      {kind === "image" && (
        <button
          type="button"
          aria-expanded={open}
          aria-label={open ? t("mediaView.shrink", "View smaller: {label}", { label }) : t("mediaView.enlarge", "View larger: {label}", { label })}
          onClick={() => setOpen(!open)}
          className="inline-flex max-w-full cursor-pointer rounded-well transition-control focus-ring"
        >
          {open ? (
            <img src={src} alt={label} className="max-h-480 max-w-full rounded-well object-contain image-outline" />
          ) : (
            <Thumbnail src={src} alt={label} />
          )}
        </button>
      )}
      {kind === "audio" && <audio controls preload="metadata" src={src} aria-label={label} className={classes} />}
      {kind === "video" && <video controls preload="metadata" playsInline src={src} aria-label={label} className={classes} />}
      <figcaption className="flex min-w-0 flex-wrap items-baseline gap-x-12 gap-y-4 text-small text-fg-secondary">
        <span className="min-w-0 wrap-anywhere">{detail ? t("mediaView.caption", "{label} ({detail})", { label, detail }) : label}</span>
        {link}
      </figcaption>
    </figure>
  );
});
MediaView.displayName = "MediaView";
