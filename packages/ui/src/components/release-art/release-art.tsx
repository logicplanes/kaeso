import { createElement, forwardRef, useId, useMemo, type ReactNode } from "react";
import { font } from "@logicplanes/kaeso-tokens";
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
  title?: ReactNode | undefined;
  eyebrow?: ReactNode | undefined;
  shape?: keyof typeof KAESO_RELEASE_ART_VARIANTS.shape | undefined;
  className?: string | undefined;
}

function toElement(n: ArtNode, key: number): ReactNode {
  return createElement(n.tag, { ...n.attrs, key }, n.text ?? n.children?.map(toElement));
}

export const ReleaseArt = forwardRef<SVGSVGElement, ReleaseArtProps>(function ReleaseArt(
  { seed, label, art, title, eyebrow, shape = "banner", className },
  ref,
) {
  const { w, h } = RELEASE_ART_VIEW[shape];
  const prefix = `art${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const spec = art ?? releaseArtFor(seed);
  const drawn = useMemo(() => drawReleaseArt(spec, { shape, idPrefix: prefix }), [spec, shape, prefix]);
  const text = releaseArtText(spec);
  return (
    <span className={cn("relative block w-full", className)}>
      <svg
        ref={ref}
        viewBox={`0 0 ${w} ${h}`}
        role="img"
        aria-label={label}
        preserveAspectRatio="xMidYMid slice"
        className={cn("block w-full overflow-hidden rounded-well", KAESO_RELEASE_ART_VARIANTS.shape[shape].classes)}
      >
        {drawn.children?.map(toElement)}
        {eyebrow && (
          <text
            x={text.eyebrow.x}
            y={h - text.eyebrow.bottom}
            fill={text.color}
            fillOpacity={0.7}
            fontSize={16}
            fontWeight={500}
            letterSpacing={0.1}
            fontFamily={font.sans}
          >
            {eyebrow}
          </text>
        )}
        {title && (
          <text x={text.title.x} y={h - text.title.bottom} fill={text.color} fontSize={52} fontWeight={600} letterSpacing={-1.5} fontFamily={font.sans}>
            {title}
          </text>
        )}
      </svg>
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
