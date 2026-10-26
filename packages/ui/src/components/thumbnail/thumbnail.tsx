import { forwardRef, useLayoutEffect, useRef, useState } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_THUMBNAIL_VARIANTS = {
  size: {
    sm: { classes: "size-64", description: "64px: a row or a compact list" },
    md: { classes: "size-120", description: "120px: an image inside a message (default)" },
    lg: { classes: "size-160", description: "160px: an image a page is about" },
  },
} as const;
export const KAESO_THUMBNAIL_DEFAULT_VARIANTS = { size: "md" } as const;

const FALLBACK = { sm: "p-4 text-caption", md: "p-8 text-caption", lg: "p-12 text-caption" } as const;
const LINES = { sm: "line-clamp-2", md: "line-clamp-3", lg: "line-clamp-3" } as const;

export interface ThumbnailProps {
  src: string;
  alt: string;
  size?: keyof typeof KAESO_THUMBNAIL_VARIANTS.size | undefined;
  className?: string | undefined;
}

export const Thumbnail = forwardRef<HTMLSpanElement, ThumbnailProps>(function Thumbnail({ src, alt, size = "md", className }, ref) {
  const [failedSrc, setFailedSrc] = useState<string | undefined>(undefined);
  const img = useRef<HTMLImageElement | null>(null);
  useLayoutEffect(() => {
    const el = img.current;
    if (el?.complete && el.naturalWidth === 0 && el.getAttribute("src")) setFailedSrc(src);
  }, [src]);
  return (
    <span
      ref={ref}
      className={cn("relative inline-flex shrink-0 overflow-hidden rounded-well bg-fill-1", KAESO_THUMBNAIL_VARIANTS.size[size].classes, className)}
    >
      {src !== failedSrc ? (
        <img ref={img} src={src} alt={alt} loading="lazy" decoding="async" className="size-full object-cover" onError={() => setFailedSrc(src)} />
      ) : (
        <span
          role="img"
          aria-label={alt}
          className={cn("flex size-full items-center justify-center overflow-hidden text-center text-fg-tertiary wrap-anywhere", FALLBACK[size])}
        >
          <span className={LINES[size]}>{alt}</span>
        </span>
      )}
      <span aria-hidden className="pointer-events-none absolute inset-0 rounded-well shadow-ring" />
    </span>
  );
});
Thumbnail.displayName = "Thumbnail";
