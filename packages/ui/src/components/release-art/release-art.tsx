import { forwardRef, useId, useMemo, type ReactNode } from "react";
import { font, neutral } from "@logicplanes/kaeso-tokens";
import { cn } from "../../utils/cn.js";

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
  title?: ReactNode | undefined;
  eyebrow?: ReactNode | undefined;
  shape?: keyof typeof KAESO_RELEASE_ART_VARIANTS.shape | undefined;
  className?: string | undefined;
}

const VIEW = { banner: { w: 640, h: 360 }, wide: { w: 840, h: 360 } } as const;
const LINES = 34;

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function random(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface ReleaseArtLine {
  d: string;
  opacity: number;
  width: number;
}

export interface ReleaseArtDrawing {
  lines: ReleaseArtLine[];
  glow: { x: number; y: number; r: number };
}

const round = (n: number) => Math.round(n * 10) / 10;

export function drawReleaseArt(seed: string, width: number, height: number): ReleaseArtDrawing {
  const next = random(hash(seed));
  const between = (lo: number, hi: number) => lo + next() * (hi - lo);
  const wave = { a: between(28, 70), f: between(0.004, 0.011), p: between(0, Math.PI * 2) };
  const swell = { a: between(10, 34), f: between(0.012, 0.024), p: between(0, Math.PI * 2) };
  const twist = between(0.06, 0.16);
  const spread = between(1.6, 3.4);
  const tilt = between(-0.18, 0.18);
  const center = between(0.32, 0.62) * height;
  const band = between(0.3, 0.7) * LINES;
  const lines: ReleaseArtLine[] = [];
  for (let i = 0; i < LINES; i++) {
    const offset = (i - LINES / 2) * spread;
    const points: string[] = [];
    for (let x = -20; x <= width + 20; x += 16) {
      const y =
        center +
        offset * (1 + Math.sin(x * 0.006 + i * twist) * 0.6) +
        Math.sin(x * wave.f + wave.p + i * twist) * wave.a +
        Math.sin(x * swell.f + swell.p - i * twist * 0.5) * swell.a +
        (x - width / 2) * tilt;
      points.push(`${round(x)} ${round(y)}`);
    }
    const near = 1 - Math.min(1, Math.abs(i - band) / (LINES * 0.55));
    lines.push({ d: `M${points.join("L")}`, opacity: round(0.06 + near * near * 0.62), width: round(0.6 + near * 0.9) });
  }
  return { lines, glow: { x: round(between(0.2, 0.8) * width), y: round(between(0.15, 0.6) * height), r: round(between(0.45, 0.75) * width) } };
}

export const ReleaseArt = forwardRef<SVGSVGElement, ReleaseArtProps>(function ReleaseArt({ seed, label, title, eyebrow, shape = "banner", className }, ref) {
  const { w, h } = VIEW[shape];
  const drawing = useMemo(() => drawReleaseArt(seed, w, h), [seed, w, h]);
  const id = useId().replace(/:/g, "");
  return (
    <svg
      ref={ref}
      viewBox={`0 0 ${w} ${h}`}
      role="img"
      aria-label={label}
      preserveAspectRatio="xMidYMid slice"
      className={cn("block w-full overflow-hidden rounded-well bg-ink text-on-ink", KAESO_RELEASE_ART_VARIANTS.shape[shape].classes, className)}
    >
      <defs>
        <radialGradient id={`${id}-glow`} cx={drawing.glow.x} cy={drawing.glow.y} r={drawing.glow.r} gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="currentColor" stopOpacity="0.22" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={`${id}-fade`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0.55" stopColor="currentColor" stopOpacity="0" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0.1" />
        </linearGradient>
      </defs>
      <rect width={w} height={h} fill={`url(#${id}-glow)`} />
      <g fill="none" stroke="currentColor" strokeLinecap="round">
        {drawing.lines.map((l, i) => (
          <path key={i} d={l.d} strokeOpacity={l.opacity} strokeWidth={l.width} />
        ))}
      </g>
      <rect width={w} height={h} fill={`url(#${id}-fade)`} />
      {eyebrow && (
        <text x="36" y={h - 92} fill="currentColor" fillOpacity="0.72" fontSize="17" fontWeight="500">
          {eyebrow}
        </text>
      )}
      {title && (
        <text x="34" y={h - 40} fill="currentColor" fontSize="52" fontWeight="600" letterSpacing="-1.5">
          {title}
        </text>
      )}
    </svg>
  );
});
ReleaseArt.displayName = "ReleaseArt";

const escapeXml = (text: string) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export interface ReleaseArtSvgOptions {
  seed: string;
  label: string;
  title?: string | undefined;
  eyebrow?: string | undefined;
  shape?: keyof typeof KAESO_RELEASE_ART_VARIANTS.shape | undefined;
  theme?: "light" | "dark" | undefined;
}

export function releaseArtSvg({ seed, label, title, eyebrow, shape = "banner", theme = "light" }: ReleaseArtSvgOptions): string {
  const { w, h } = VIEW[shape];
  const d = drawReleaseArt(seed, w, h);
  const ink = neutral.ink[theme];
  const onInk = neutral["on-ink"][theme];
  const family = escapeXml(font.sans);
  const lines = d.lines.map((l) => `<path d="${l.d}" stroke-opacity="${l.opacity}" stroke-width="${l.width}"/>`).join("");
  const text = [
    eyebrow
      ? `<text x="36" y="${h - 92}" fill="${onInk}" fill-opacity="0.72" font-size="17" font-weight="500" font-family="${family}">${escapeXml(eyebrow)}</text>`
      : "",
    title
      ? `<text x="34" y="${h - 40}" fill="${onInk}" font-size="52" font-weight="600" letter-spacing="-1.5" font-family="${family}">${escapeXml(title)}</text>`
      : "",
  ].join("");
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w * 2}" height="${h * 2}" role="img" aria-label="${escapeXml(label)}">`,
    `<defs><radialGradient id="glow" cx="${d.glow.x}" cy="${d.glow.y}" r="${d.glow.r}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="${onInk}" stop-opacity="0.22"/><stop offset="1" stop-color="${onInk}" stop-opacity="0"/></radialGradient>`,
    `<linearGradient id="veil" x1="0" y1="0" x2="0" y2="1"><stop offset="0.55" stop-color="${onInk}" stop-opacity="0"/><stop offset="1" stop-color="${onInk}" stop-opacity="0.1"/></linearGradient></defs>`,
    `<rect width="${w}" height="${h}" fill="${ink}"/><rect width="${w}" height="${h}" fill="url(#glow)"/>`,
    `<g fill="none" stroke="${onInk}" stroke-linecap="round">${lines}</g>`,
    `<rect width="${w}" height="${h}" fill="url(#veil)"/>${text}</svg>`,
  ].join("");
}
