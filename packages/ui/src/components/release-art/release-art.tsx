import { createElement, forwardRef, useId, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useLayoutWidth } from "../../hooks/use-layout-width.js";
import { cn } from "../../utils/cn.js";
import { drawReleaseArt, releaseArtFor, releaseArtText, serializeArt, RELEASE_ART_VIEW, type ArtNode, type ReleaseArtSpec } from "./drawing.js";

export { RELEASE_ART_STYLES, isReleaseArtSpec, nextReleaseArt, randomReleaseArt, releaseArtFor, type ReleaseArtSpec, type ReleaseArtStyle } from "./drawing.js";

export const KAESO_RELEASE_ART_VARIANTS = {
  shape: {
    banner: { classes: "aspect-video", description: "16:9: the head of a dialog or a card (default)" },
    wide: { classes: "aspect-21/9", description: "21:9: a wide strip above a page's content" },
  },
} as const;
export const KAESO_RELEASE_ART_DEFAULT_VARIANTS = { shape: "banner" } as const;

export interface ReleaseArtProps {
  seed: string;
  label: string;
  art?: ReleaseArtSpec | undefined;
  src?: string | undefined;
  title?: ReactNode | undefined;
  eyebrow?: ReactNode | undefined;
  shape?: keyof typeof KAESO_RELEASE_ART_VARIANTS.shape | undefined;
  className?: string | undefined;
}

function toElement(n: ArtNode, key: number): ReactNode {
  return createElement(n.tag, { ...n.attrs, key }, n.text ?? n.children?.map(toElement));
}

const TITLE_SIZES = ["text-display", "text-title-lg", "text-title", "text-title-sm"] as const;
const WRAP = "text-title-sm";

function useTitleFit(ref: { current: HTMLSpanElement | null }, width: number | null, title: string): string | null {
  const start = width === null ? null : width >= 512 ? 0 : width >= 320 ? 1 : width >= 208 ? 2 : 3;
  const [step, setStep] = useState<{ key: string; at: number } | null>(null);
  const key = `${start}|${title}`;
  const at = start === null ? null : step?.key === key ? step.at : start;
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || at === null || at >= TITLE_SIZES.length - 1) return;
    if (el.scrollWidth > el.clientWidth + 1) setStep({ key, at: at + 1 });
  }, [at, key, ref]);
  return at === null ? null : TITLE_SIZES[at]!;
}

export const ReleaseArt = forwardRef<SVGSVGElement, ReleaseArtProps>(function ReleaseArt(
  { seed, label, art, src, title, eyebrow, shape = "banner", className },
  ref,
) {
  const { w, h } = RELEASE_ART_VIEW[shape];
  const [failed, setFailed] = useState<string | null>(null);
  const prefix = `art${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const spec = art ?? releaseArtFor(seed);
  const drawn = useMemo(() => drawReleaseArt(spec, { shape, idPrefix: prefix }), [spec, shape, prefix]);
  const text = releaseArtText(spec);
  const clear = 236;
  const frame = useRef<HTMLSpanElement | null>(null);
  const heading = useRef<HTMLSpanElement | null>(null);
  const width = useLayoutWidth(frame);
  const fit = useTitleFit(heading, width, typeof title === "string" ? title : "");
  return (
    <span ref={frame} className={cn("@container relative block w-full", className)}>
      <svg
        ref={ref}
        viewBox={`0 0 ${w} ${h}`}
        role="img"
        aria-label={label}
        preserveAspectRatio="xMidYMid slice"
        className={cn("block w-full overflow-hidden rounded-well", KAESO_RELEASE_ART_VARIANTS.shape[shape].classes)}
      >
        {src && failed !== src ? (
          <image href={src} width={w} height={h} preserveAspectRatio="xMidYMid slice" onError={() => setFailed(src)} />
        ) : (
          drawn.children?.map(toElement)
        )}
      </svg>
      {(eyebrow || title) && (
        <span
          aria-hidden
          className="pointer-events-none absolute flex flex-col items-start gap-4 text-balance"
          style={{
            left: `${(text.eyebrow.x / w) * 100}cqw`,
            bottom: `${(text.title.bottom / w) * 100}cqw`,
            maxWidth: `${(clear / w) * 100}cqw`,
            hyphens: "auto",
            color: text.color,
          }}
        >
          {eyebrow && <span className="text-caption font-medium opacity-70 @cell-lg:text-body-lg">{eyebrow}</span>}
          {title && (
            <span
              ref={heading}
              className={cn(
                "max-w-full font-semibold",
                fit ?? "text-title-sm @cell-sm:text-title @cell-md:text-title-lg @cell-lg:text-display",
                fit && fit !== WRAP && "whitespace-nowrap",
              )}
            >
              {title}
            </span>
          )}
        </span>
      )}
      <span aria-hidden className="pointer-events-none absolute inset-0 rounded-well shadow-ring" />
    </span>
  );
});
ReleaseArt.displayName = "ReleaseArt";

export interface ReleaseArtSvgOptions {
  seed: string;
  label: string;
  art?: ReleaseArtSpec | undefined;
  title?: string | undefined;
  eyebrow?: string | undefined;
  shape?: keyof typeof KAESO_RELEASE_ART_VARIANTS.shape | undefined;
}

export function releaseArtSvg({ seed, label, art, title, eyebrow, shape = "banner" }: ReleaseArtSvgOptions): string {
  const { w, h } = RELEASE_ART_VIEW[shape];
  const drawn = drawReleaseArt(art ?? releaseArtFor(seed), { shape, title, eyebrow, idPrefix: "art" });
  return serializeArt({ ...drawn, attrs: { ...drawn.attrs, width: w * 2, height: h * 2, role: "img", ariaLabel: label } });
}
