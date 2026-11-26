import { art, font } from "@logicplanes/kaeso-tokens";

// The flat geometric styles, of earlier releases, kept for their stored artwork.
export const RELEASE_ART_FLAT_STYLES = ["bauhaus", "albers", "stripes", "waves", "truchet", "arcs", "phyllotaxis", "spiro"] as const;
// Every style a spec may name: the flat ones and `glow`, a release's name in white on a dark ground lit by one bloom
// of light, the style of every new release.
export const RELEASE_ART_STYLES = [...RELEASE_ART_FLAT_STYLES, "glow"] as const;
export type ReleaseArtStyle = (typeof RELEASE_ART_STYLES)[number];

export interface ReleaseArtSpec {
  version: 1;
  seed: number;
  style: ReleaseArtStyle;
  palette: number;
}

export interface ArtNode {
  tag: string;
  attrs: Record<string, string | number>;
  children?: ArtNode[] | undefined;
  text?: string | undefined;
}

export const RELEASE_ART_VIEW = { banner: { w: 640, h: 360 }, wide: { w: 840, h: 360 }, strip: { w: 1200, h: 240 } } as const;
export type ReleaseArtShape = keyof typeof RELEASE_ART_VIEW;

const U = 40;
// The corner kept clear for words: a title on a cover, and on a strip the whole text of the surface the art lies behind.
const CLEAR = { banner: { rx: U * 7.5, ry: U * 7.5 }, wide: { rx: U * 7.5, ry: U * 7.5 }, strip: { rx: U * 18, ry: U * 18 } } as const;

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function random(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function isReleaseArtSpec(value: unknown): value is ReleaseArtSpec {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    v.version === 1 &&
    typeof v.seed === "number" &&
    Number.isInteger(v.seed) &&
    v.seed >= 0 &&
    v.seed <= 0xffffffff &&
    typeof v.style === "string" &&
    (RELEASE_ART_STYLES as readonly string[]).includes(v.style) &&
    typeof v.palette === "number" &&
    Number.isInteger(v.palette) &&
    v.palette >= 0 &&
    v.palette < art.palettes.length
  );
}

export function randomReleaseArt(next: () => number = Math.random): ReleaseArtSpec {
  const pick = (n: number) => Math.min(n - 1, Math.floor(next() * n));
  return {
    version: 1,
    seed: Math.min(0xffffffff, Math.floor(next() * 0x100000000)),
    style: RELEASE_ART_STYLES[pick(RELEASE_ART_STYLES.length)] ?? "glow",
    palette: pick(art.palettes.length),
  };
}

// A new release's artwork: a glow whose light differs from the last four releases', with a seed no release had.
export function nextReleaseArt(previous: readonly ReleaseArtSpec[], next: () => number = Math.random): ReleaseArtSpec {
  const palettes = art.palettes.map((_, i) => i).filter((i) => !previous.slice(-4).some((p) => p.palette === i));
  const seeds = new Set(previous.map((p) => p.seed));
  const pick = <T>(list: readonly T[]) => list[Math.min(list.length - 1, Math.floor(next() * list.length))];
  let seed = Math.min(0xffffffff, Math.floor(next() * 0x100000000));
  while (seeds.has(seed)) seed = (seed + 1) >>> 0;
  return { version: 1, seed, style: "glow", palette: pick(palettes) ?? 0 };
}

// The artwork of a release without a stored one: a glow drawn from its version.
export function releaseArtFor(text: string): ReleaseArtSpec {
  const seed = hash(text);
  return { version: 1, seed, style: "glow", palette: (seed >>> 8) % art.palettes.length };
}

const round = (n: number) => Math.round(n * 100) / 100;
const node = (tag: string, attrs: ArtNode["attrs"], children?: ArtNode[]): ArtNode => (children ? { tag, attrs, children } : { tag, attrs });

interface Canvas {
  w: number;
  h: number;
  cols: number;
  rows: number;
  next: () => number;
  between: (lo: number, hi: number) => number;
  int: (lo: number, hi: number) => number;
  chance: (p: number) => boolean;
  ground: string;
  clear: { rx: number; ry: number };
  color: (i: number) => string;
  id: (name: string) => string;
}

const open = (c: Canvas, x: number, y: number) => (x / c.clear.rx) ** 2 + ((c.h - y) / c.clear.ry) ** 2 < 1;
const clearEdge = (c: Canvas, y: number) => {
  const dy = (c.h - y) / c.clear.ry;
  return dy >= 1 ? 0 : c.clear.rx * Math.sqrt(1 - dy * dy);
};
const masked = (c: Canvas, children: ArtNode[]) => node("g", { mask: `url(#${c.id("clear")})` }, children);

function primitive(c: Canvas, x: number, y: number, s: number, fill: string, accent: string): ArtNode[] {
  const out: ArtNode[] = [];
  const kind = c.int(0, 7);
  const t = `rotate(${c.int(0, 3) * 90} ${round(x + s / 2)} ${round(y + s / 2)})`;
  if (c.chance(0.55)) out.push(node("rect", { x, y, width: s, height: s, fill }));
  const ink = out.length ? accent : fill;
  switch (kind) {
    case 0:
      out.push(node("path", { d: `M${x} ${y}L${x + s} ${y}A${s} ${s} 0 0 1 ${x} ${y + s}Z`, fill: ink, transform: t }));
      break;
    case 1:
      out.push(node("path", { d: `M${x} ${y + s}A${s / 2} ${s / 2} 0 0 1 ${x + s} ${y + s}Z`, fill: ink, transform: t }));
      break;
    case 2:
      out.push(node("circle", { cx: x + s / 2, cy: y + s / 2, r: s / 2, fill: ink }));
      break;
    case 3:
      out.push(node("path", { d: `M${x} ${y}L${x + s} ${y + s}L${x} ${y + s}Z`, fill: ink, transform: t }));
      break;
    case 4:
      out.push(node("circle", { cx: x + s / 2, cy: y + s / 2, r: s / 4, fill: ink }));
      break;
    case 5:
      for (let i = 0; i < 3; i++) out.push(node("rect", { x: x + (i * s) / 3, y, width: s / 6, height: s, fill: ink, transform: t }));
      break;
    case 6:
      out.push(node("path", { d: `M${x} ${y}L${x + s} ${y}L${x + s / 2} ${y + s / 2}Z`, fill: ink, transform: t }));
      out.push(node("path", { d: `M${x} ${y + s}L${x + s} ${y + s}L${x + s / 2} ${y + s / 2}Z`, fill: ink, transform: t }));
      break;
    default:
      out.push(node("path", { d: `M${x} ${y}A${s} ${s} 0 0 1 ${x + s} ${y + s}L${x} ${y + s}Z`, fill: ink, transform: t }));
  }
  return out;
}

function bauhaus(c: Canvas): ArtNode[] {
  const taken = new Set<string>();
  const key = (col: number, row: number) => `${col},${row}`;
  const blocked = (col: number, row: number) => open(c, col * U, (row + 1) * U);
  const out: ArtNode[] = [];
  const bigs = c.int(2, 4);
  for (let b = 0; b < bigs; b++) {
    const size = c.int(2, 3);
    for (let attempt = 0; attempt < 20; attempt++) {
      const col = c.int(0, c.cols - size);
      const row = c.int(0, c.rows - size);
      let free = true;
      for (let i = 0; i < size && free; i++) for (let j = 0; j < size && free; j++) free = !taken.has(key(col + i, row + j)) && !blocked(col + i, row + j);
      if (!free) continue;
      for (let i = 0; i < size; i++) for (let j = 0; j < size; j++) taken.add(key(col + i, row + j));
      const a = c.int(0, 3);
      out.push(...primitive(c, col * U, row * U, size * U, c.color(a), c.color(a + c.int(1, 3))));
      break;
    }
  }
  const density = c.between(0.55, 0.85);
  for (let col = 0; col < c.cols; col++) {
    for (let row = 0; row < c.rows; row++) {
      if (taken.has(key(col, row)) || blocked(col, row) || !c.chance(density)) continue;
      const a = c.int(0, 3);
      out.push(...primitive(c, col * U, row * U, U, c.color(a), c.color(a + c.int(1, 3))));
    }
  }
  return out;
}

function albers(c: Canvas): ArtNode[] {
  const side = U * 6;
  const margin = (c.h - side) / 2;
  const lo = c.clear.rx - U / 2;
  const hi = c.w - margin - side;
  const x0 = lo + Math.round((c.next() * Math.max(0, hi - lo)) / (U / 2)) * (U / 2);
  const unit = side / 20;
  const levels = c.int(3, 4);
  const order = [0, 1, 2, 3];
  for (let i = order.length - 1; i > 0; i--) {
    const j = Math.floor(c.next() * (i + 1));
    [order[i], order[j]] = [order[j] ?? j, order[i] ?? i];
  }
  return Array.from({ length: levels }, (_, l) =>
    node("rect", { x: x0 + 2 * unit * l, y: margin + 3 * unit * l, width: side - 4 * unit * l, height: side - 4 * unit * l, fill: c.color(order[l] ?? l) }),
  );
}

function bands(c: Canvas, angle: number, widths: number[], offset: number): ArtNode {
  const diag = Math.hypot(c.w, c.h);
  const rects: ArtNode[] = [];
  let x = -diag;
  let i = 0;
  while (x < diag) {
    const w = widths[i % widths.length] ?? U;
    rects.push(node("rect", { x: round(x), y: round(-diag), width: w, height: round(diag * 2), fill: c.color(i + offset) }));
    x += w;
    i += 1;
  }
  return node("g", { transform: `translate(${c.w / 2} ${c.h / 2}) rotate(${angle})` }, rects);
}

function stripes(c: Canvas): ArtNode[] {
  const angle = [0, 90, 45, -45, 30, -60][c.int(0, 5)] ?? 0;
  const widths = Array.from({ length: c.int(5, 9) }, () => (U / 2) * c.int(1, 5));
  const clip = c.id("disc");
  const r = U * c.int(2, 3) + U / 2;
  const cx = U * c.int(Math.ceil((c.clear.rx + r) / U), c.cols - 2);
  const cy = U * c.int(3, c.rows - 3);
  return [
    node("defs", {}, [node("clipPath", { id: clip }, [node("circle", { cx, cy, r })])]),
    masked(c, [bands(c, angle, widths, 0)]),
    node("g", { clipPath: `url(#${clip})` }, [node("rect", { width: c.w, height: c.h, fill: c.ground }), bands(c, angle + 90, widths, 1)]),
  ];
}

function waves(c: Canvas): ArtNode[] {
  const out: ArtNode[] = [];
  const lines = c.int(14, 18);
  const top = U * 2.5;
  const bottom = c.h - U;
  const gapY = (bottom - top) / (lines - 1);
  const peak = c.w * c.between(0.58, 0.74);
  const spread = c.w * c.between(0.09, 0.14);
  const accent = c.chance(0.6) ? c.int(2, lines - 3) : -1;
  for (let i = 0; i < lines; i++) {
    const base = top + i * gapY;
    const bumps = Array.from({ length: c.int(3, 5) }, () => ({ x: peak + c.between(-1.2, 1.2) * spread, h: c.between(0.3, 1), s: c.between(9, 22) }));
    const ripple = [c.between(0, 6.28), c.between(0, 6.28)];
    const start = Math.max(U, Math.ceil((clearEdge(c, base) + U / 2) / 4) * 4);
    const raw: [number, number][] = [];
    for (let x = start; x <= c.w - U; x += 4) {
      const env = Math.exp(-((x - peak) ** 2) / (2 * spread * spread));
      let y = 0;
      for (const b of bumps) y += b.h * Math.exp(-((x - b.x) ** 2) / (2 * b.s * b.s));
      const fine = Math.sin(x * 0.21 + (ripple[0] ?? 0)) * 0.6 + Math.sin(x * 0.53 + (ripple[1] ?? 0)) * 0.4;
      raw.push([x, y * env + fine * 0.04 * (0.15 + env)]);
    }
    const max = Math.max(...raw.map(([, y]) => y), 0.001);
    const lift = Math.min(gapY * c.between(2.4, 4.4), base - U);
    const line = `M${raw.map(([x, y]) => `${x} ${round(base - (y / max) * lift)}`).join("L")}`;
    out.push(node("path", { d: `${line}L${c.w - U} ${round(base + gapY)}L${start} ${round(base + gapY)}Z`, fill: c.ground }));
    out.push(
      node("path", { d: line, fill: "none", stroke: i === accent ? c.color(1) : c.color(0), strokeWidth: 2, strokeLinejoin: "round", strokeLinecap: "round" }),
    );
  }
  return out;
}

function truchet(c: Canvas): ArtNode[] {
  const t = U * 2;
  const h = t / 2;
  const stroke = round(t * c.between(0.2, 0.3));
  const inner = c.chance(0.5);
  const paths: ArtNode[] = [];
  const cols = Math.ceil(c.w / t);
  const rows = Math.ceil(c.h / t);
  for (let col = 0; col < cols; col++) {
    for (let r = 0; r < rows; r++) {
      const x = col * t;
      const y = c.h - (r + 1) * t;
      if (open(c, x, y + t)) continue;
      const d = c.chance(0.5)
        ? `M${x + h} ${y}A${h} ${h} 0 0 1 ${x} ${y + h}M${x + t} ${y + h}A${h} ${h} 0 0 0 ${x + h} ${y + t}`
        : `M${x + h} ${y}A${h} ${h} 0 0 0 ${x + t} ${y + h}M${x} ${y + h}A${h} ${h} 0 0 1 ${x + h} ${y + t}`;
      paths.push(node("path", { d }));
    }
  }
  const out = [node("g", { fill: "none", stroke: c.color(0), strokeWidth: stroke }, paths)];
  if (inner) out.push(node("g", { fill: "none", stroke: c.color(1), strokeWidth: round(stroke * 0.34) }, paths));
  return out;
}

function clearance(c: Canvas, cx: number, cy: number): number {
  let best = Infinity;
  for (let i = 0; i <= 90; i++) {
    const a = (i / 90) * (Math.PI / 2);
    best = Math.min(best, Math.hypot(cx - c.clear.rx * Math.cos(a), cy - (c.h - c.clear.ry * Math.sin(a))));
  }
  return open(c, cx, cy) ? 0 : best;
}

function arcs(c: Canvas): ArtNode[] {
  const out: ArtNode[] = [];
  const anchors = c.int(2, 3);
  const spots: [number, number][] = [
    [U * c.int(4, c.cols - 2), 0],
    [U * c.int(11, c.cols - 1), c.h],
    [c.w, U * c.int(2, c.rows - 2)],
    [c.w, 0],
    [c.w, c.h],
    [0, 0],
  ];
  for (let i = spots.length - 1; i > 0; i--) {
    const j = Math.floor(c.next() * (i + 1));
    [spots[i], spots[j]] = [spots[j] ?? [c.w, 0], spots[i] ?? [c.w, 0]];
  }
  let placed = 0;
  for (const [cx, cy] of spots) {
    if (placed === anchors) break;
    const band = (U / 2) * c.int(1, 2);
    const room = Math.floor((clearance(c, cx, cy) - U / 2) / band);
    const count = Math.min(room, c.int(4, 10));
    if (count < 3) continue;
    const offset = c.int(0, 3);
    const hollow = c.chance(0.4);
    for (let i = count; i >= 1; i--) out.push(node("circle", { cx, cy, r: i * band, fill: i === 1 && hollow ? c.ground : c.color(i + offset) }));
    placed += 1;
  }
  return out;
}

const GOLDEN = 2.399963229728653;

function phyllotaxis(c: Canvas): ArtNode[] {
  const out: ArtNode[] = [];
  const cx = c.w - c.h / 2 - U * c.int(0, 2);
  const cy = c.h / 2 + U * c.int(-1, 1);
  const step = c.between(11, 14);
  const arms = [8, 13, 21][c.int(0, 2)] ?? 13;
  const banded = c.chance(0.35);
  const reach = Math.hypot(c.w, c.h);
  for (let i = 1; ; i++) {
    const r = step * Math.sqrt(i);
    if (r > reach) break;
    const x = cx + r * Math.cos(i * GOLDEN);
    const y = cy + r * Math.sin(i * GOLDEN);
    const size = step * (0.3 + 0.12 * Math.min(1, r / (c.h / 2)));
    if (x < -size || x > c.w + size || y < -size || y > c.h + size) continue;
    if (open(c, x, y) || clearance(c, x, y) < size + U / 4) continue;
    const tone = banded ? Math.floor(r / (U * 1.5)) % 4 : Math.floor(((i % arms) / arms) * 4);
    out.push(node("circle", { cx: round(x), cy: round(y), r: round(size), fill: c.color(tone) }));
  }
  return out;
}

function gcd(a: number, b: number): number {
  return b ? gcd(b, a % b) : a;
}

function spiro(c: Canvas): ArtNode[] {
  const out: ArtNode[] = [];
  const cx = c.w - c.h / 2;
  const cy = c.h / 2;
  const radius = c.h / 2 - U;
  const big = c.int(7, 13);
  let small = c.int(2, big - 2);
  while (gcd(big, small) !== 1) small = c.int(2, big - 2);
  const layers = c.chance(0.5) ? 2 : 1;
  for (let l = 0; l < layers; l++) {
    const pen = small * c.between(0.75, 1.05);
    const k = (big - small) / small;
    const extent = big - small + pen;
    const turns = small / gcd(big, small);
    const steps = turns * 360;
    const points: string[] = [];
    const scale = (radius * (l === 0 ? 1 : c.between(0.4, 0.55))) / extent;
    for (let i = 0; i <= steps; i++) {
      const a = (i / steps) * turns * Math.PI * 2;
      const x = (big - small) * Math.cos(a) + pen * Math.cos(k * a);
      const y = (big - small) * Math.sin(a) - pen * Math.sin(k * a);
      points.push(`${round(cx + x * scale)} ${round(cy + y * scale)}`);
    }
    const d = `M${points.join("L")}Z`;
    out.push(node("path", { d, fill: c.color(l), fillRule: "evenodd" }));
  }
  return out;
}

const STYLE = { bauhaus, albers, stripes, waves, truchet, arcs, phyllotaxis, spiro } satisfies Record<
  (typeof RELEASE_ART_FLAT_STYLES)[number],
  (c: Canvas) => ArtNode[]
>;

export interface DrawOptions {
  shape?: ReleaseArtShape | undefined;
  title?: string | undefined;
  eyebrow?: string | undefined;
  idPrefix?: string | undefined;
}

const paletteOf = (spec: ReleaseArtSpec) => art.palettes[spec.palette % art.palettes.length] ?? art.palettes[0];

// Which of the interface's two modes reads on the artwork's ground: paper takes dark text, ink and a glow light text.
export function releaseArtScheme(spec: ReleaseArtSpec): "light" | "dark" {
  if (spec.style === "glow" || !Object.hasOwn(STYLE, spec.style)) return "dark";
  return paletteOf(spec).ground === "paper" ? "light" : "dark";
}

export function releaseArtText(spec: ReleaseArtSpec) {
  if (spec.style === "glow" || !Object.hasOwn(STYLE, spec.style)) return { color: "#ffffff", eyebrow: { x: U, bottom: 96 }, title: { x: U - 2, bottom: U } };
  return { color: art.grounds[paletteOf(spec).ground].text, eyebrow: { x: U, bottom: 96 }, title: { x: U - 2, bottom: U } };
}

// The glow: a release's name in white on a dark ground, lit by one bloom of its palette's light rising from beyond an
// edge and a fainter one from another, with a photograph's fine grain, as Kaeso's launch articles set theirs.

const toSrgb = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

function oklchToLinear([L, C, hue]: readonly [number, number, number]): [number, number, number] {
  const a = C * Math.cos((hue * Math.PI) / 180);
  const b = C * Math.sin((hue * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    clamp01(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    clamp01(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    clamp01(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ];
}

type Linear = readonly [number, number, number];
const hex = (c: Linear) =>
  `#${c
    .map((v) =>
      Math.round(clamp01(toSrgb(v)) * 255)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
const luminance = (c: Linear) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];

// The glow's light from its deepest tone (0) to its palest (1), mixed in OKLab so it stays even.
function glowRamp(palette: number): (t: number) => Linear {
  const ramp = art.glows[palette % art.glows.length] ?? art.glows[0];
  const stops = ramp.stops.map(([L, C, hue]) => [L, C * Math.cos((hue * Math.PI) / 180), C * Math.sin((hue * Math.PI) / 180)] as const);
  return (t) => {
    const at = clamp01(t) * (stops.length - 1);
    const k = Math.min(Math.floor(at), stops.length - 2);
    const f = at - k;
    const [L0, a0, b0] = stops[k] ?? stops[0]!;
    const [L1, a1, b1] = stops[k + 1] ?? stops[0]!;
    const L = L0 + (L1 - L0) * f;
    const a = a0 + (a1 - a0) * f;
    const b = b0 + (b1 - b0) * f;
    return oklchToLinear([L, Math.hypot(a, b), (Math.atan2(b, a) * 180) / Math.PI]);
  };
}

const NIGHT: Linear = [0.0018, 0.0018, 0.0018];
const GLOW_NAME = { size: 34, spacing: -0.51, weight: 500 } as const;
// A bloom reaches this many of its radii before its light is below what 8 bits show.
const GLOW_REACH = 2.3;
const GLOW_STOPS = 40;
// The least luminance the brightest edge of a glow reaches.
export const GLOW_LEAST = 0.3;

export interface Bloom {
  x: number;
  y: number;
  r: number;
  strength: number;
}

// The light a bloom adds at a distance from its centre, in linear light: the palette's deep tones at its rim, its
// pale ones at its heart.
function bloomLight(ramp: (t: number) => Linear, bloom: Bloom, d: number): Linear {
  const g = Math.exp(-((d / bloom.r) ** 2));
  const c = ramp(0.15 + 0.85 * g);
  const k = g ** 1.8 * 0.8 * bloom.strength;
  return [c[0] * k, c[1] * k, c[2] * k];
}

export interface GlowLight {
  blooms: [Bloom, Bloom];
  // The contrast of white against the brightest light where the name sits.
  contrast: number;
  // The luminance of the brightest point on the picture's top and bottom edges.
  brightest: number;
}

export function releaseArtGlowLight(spec: ReleaseArtSpec, shape: ReleaseArtShape = "banner"): GlowLight {
  const { w, h } = RELEASE_ART_VIEW[shape];
  const ramp = glowRamp(spec.palette);
  const next = random(spec.seed);
  // The blooms are drawn again until white stands out at 7:1 or more where the name sits, and the light is bright
  // enough somewhere to read as light, not as a stain.
  for (let attempt = 0; ; attempt++) {
    const top = next() < 0.5;
    // A strip's light rises on its right, large enough to fill its low height, and its fainter one on its left.
    const strip = shape === "strip";
    const main: Bloom = {
      x: w * (strip ? 0.68 + next() * 0.22 : 0.15 + next() * 0.7),
      y: h * (top ? -0.18 - next() * 0.15 : 1.18 + next() * 0.15),
      r: h * (strip ? 0.7 + next() * 0.25 : 0.42 + next() * 0.2),
      strength: 1,
    };
    const second: Bloom = { x: w * next() * (strip ? 0.3 : 1), y: h * (next() < 0.5 ? -0.3 : 1.3), r: h * (0.3 + next() * 0.2), strength: 0.3 };
    const at = (x: number, y: number) => {
      const a = bloomLight(ramp, main, Math.hypot(x - main.x, y - main.y));
      const b = bloomLight(ramp, second, Math.hypot(x - second.x, y - second.y));
      return luminance([NIGHT[0] + a[0] + b[0], NIGHT[1] + a[1] + b[1], NIGHT[2] + a[2] + b[2]]);
    };
    // A strip lies behind a surface's words, which fill its left; a cover carries its name in its middle.
    const [x0, x1, y0, y1] = strip ? [0, 0.5, 0, 1] : [0.25, 0.75, 0.38, 0.62];
    let behindName = 0;
    for (let x = w * x0; x <= w * x1; x += w / 40) for (let y = h * y0; y <= h * y1; y += h / 40) behindName = Math.max(behindName, at(x, y));
    let brightest = 0;
    for (let x = 0; x <= w; x += w / 32) for (const y of [0, h]) brightest = Math.max(brightest, at(x, y));
    const contrast = 1.05 / (behindName + 0.05);
    if ((contrast >= 7 && brightest >= GLOW_LEAST) || attempt >= 63) return { blooms: [main, second], contrast, brightest };
  }
}

function glowGradient(id: string, ramp: (t: number) => Linear, bloom: Bloom, ground: Linear | null): ArtNode {
  const reach = bloom.r * GLOW_REACH;
  const stops: ArtNode[] = [];
  for (let i = 0; i <= GLOW_STOPS; i++) {
    // Denser where the light turns fastest, near the heart.
    const t = (i / GLOW_STOPS) ** 1.35;
    const light = bloomLight(ramp, bloom, t * reach);
    if (ground) {
      stops.push(node("stop", { offset: round(t * 10000) / 10000, stopColor: hex([ground[0] + light[0], ground[1] + light[1], ground[2] + light[2]]) }));
    } else {
      // Over the ground, a faint bloom adds its light: the colour at full strength, as much of it as it lends.
      const peak = Math.max(light[0], light[1], light[2], 1e-6);
      const alpha = clamp01(toSrgb(peak + NIGHT[0]) - toSrgb(NIGHT[0]));
      const tint: Linear = [light[0] / peak, light[1] / peak, light[2] / peak];
      stops.push(node("stop", { offset: round(t * 10000) / 10000, stopColor: hex(tint), stopOpacity: Math.round(alpha * 1000) / 1000 }));
    }
  }
  return node("radialGradient", { id, gradientUnits: "userSpaceOnUse", cx: round(bloom.x), cy: round(bloom.y), r: round(reach) }, stops);
}

export function releaseArtGlowName(eyebrow: string | undefined, title: string | undefined): string {
  return [eyebrow, title].filter((part) => part && part.trim()).join(" ");
}

// Where a glow's name sits: centred, one line, scaled down to fit four fifths of the width.
export function releaseArtGlowText(name: string, w: number, h: number) {
  const estimate = [...name].reduce((sum, ch) => sum + (/[.,:;'!|il1]/.test(ch) ? 0.3 : /\s/.test(ch) ? 0.28 : /[mwMW]/.test(ch) ? 0.85 : 0.6), 0);
  const size = Math.min(GLOW_NAME.size, (w * 0.8) / Math.max(estimate, 1));
  return { x: w / 2, y: h / 2 + size * 0.364, size: round(size), spacing: round((GLOW_NAME.spacing / GLOW_NAME.size) * size), weight: GLOW_NAME.weight };
}

function drawGlow(spec: ReleaseArtSpec, shape: ReleaseArtShape, prefix: string, name: string): ArtNode {
  const { w, h } = RELEASE_ART_VIEW[shape];
  const ramp = glowRamp(spec.palette);
  const [main, second] = releaseArtGlowLight(spec, shape).blooms;
  const id = (part: string) => `${prefix}-${part}`;
  const children: ArtNode[] = [
    node("rect", { width: w, height: h, fill: `url(#${id("bloom")})` }),
    node("rect", { width: w, height: h, fill: `url(#${id("second")})` }),
  ];
  const text = name ? releaseArtGlowText(name, w, h) : null;
  return node("svg", { xmlns: "http://www.w3.org/2000/svg", viewBox: `0 0 ${w} ${h}` }, [
    node("defs", {}, [
      glowGradient(id("bloom"), ramp, main, NIGHT),
      glowGradient(id("second"), ramp, second, null),
      node("filter", { id: id("grain"), filterUnits: "userSpaceOnUse", x: 0, y: 0, width: w, height: h, colorInterpolationFilters: "sRGB" }, [
        node("feTurbulence", { type: "fractalNoise", baseFrequency: 1.2, numOctaves: 3, seed: spec.seed % 9973, stitchTiles: "stitch", result: "noise" }),
        node("feColorMatrix", { in: "noise", type: "matrix", values: "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 0 1", result: "grey" }),
        node("feComposite", { in: "SourceGraphic", in2: "grey", operator: "arithmetic", k1: 0, k2: 1, k3: 0.045, k4: -0.0225 }),
      ]),
    ]),
    node("g", { filter: `url(#${id("grain")})` }, children),
    ...(text
      ? [
          {
            tag: "text",
            attrs: {
              x: round(text.x),
              y: round(text.y),
              fill: "#ffffff",
              fontSize: text.size,
              fontWeight: text.weight,
              letterSpacing: text.spacing,
              fontFamily: font.sans,
              textAnchor: "middle",
            },
            text: name,
          },
        ]
      : []),
  ]);
}

export function drawReleaseArt(spec: ReleaseArtSpec, options: DrawOptions = {}): ArtNode {
  const { w, h } = RELEASE_ART_VIEW[options.shape ?? "banner"];
  // A style this version doesn't know, as from a newer update service, draws as a glow rather than failing.
  if (spec.style === "glow" || !Object.hasOwn(STYLE, spec.style))
    return drawGlow(spec, options.shape ?? "banner", options.idPrefix ?? "art", releaseArtGlowName(options.eyebrow, options.title));
  const next = random(spec.seed);
  const between = (lo: number, hi: number) => lo + next() * (hi - lo);
  const int = (lo: number, hi: number) => Math.min(hi, lo + Math.floor(next() * (hi - lo + 1)));
  const palette = paletteOf(spec);
  const place = releaseArtText(spec);
  const ground = art.grounds[palette.ground];
  const colors: string[] = [...palette.colors];
  for (let i = colors.length - 1; i > 0; i--) {
    const j = Math.floor(next() * (i + 1));
    [colors[i], colors[j]] = [colors[j] ?? ground.text, colors[i] ?? ground.text];
  }
  const prefix = options.idPrefix ?? "art";
  const canvas: Canvas = {
    w,
    h,
    cols: w / U,
    rows: h / U,
    next,
    between,
    int,
    chance: (p) => next() < p,
    ground: ground.fill,
    clear: CLEAR[options.shape ?? "banner"],
    color: (i) => colors[((i % colors.length) + colors.length) % colors.length] ?? ground.text,
    id: (name) => `${prefix}-${name}`,
  };
  const mask = canvas.id("clear");
  const drawn = STYLE[spec.style as (typeof RELEASE_ART_FLAT_STYLES)[number]](canvas);
  const text: ArtNode[] = [];
  if (options.eyebrow)
    text.push({
      tag: "text",
      attrs: {
        x: place.eyebrow.x,
        y: h - place.eyebrow.bottom,
        fill: place.color,
        fillOpacity: 0.7,
        fontSize: 16,
        fontWeight: 500,
        letterSpacing: 0.1,
        fontFamily: font.sans,
      },
      text: options.eyebrow,
    });
  if (options.title)
    text.push({
      tag: "text",
      attrs: { x: place.title.x, y: h - place.title.bottom, fill: place.color, fontSize: 52, fontWeight: 600, letterSpacing: -1.5, fontFamily: font.sans },
      text: options.title,
    });
  return node("svg", { xmlns: "http://www.w3.org/2000/svg", viewBox: `0 0 ${w} ${h}` }, [
    node("defs", {}, [
      node("mask", { id: mask, maskUnits: "userSpaceOnUse", x: 0, y: 0, width: w, height: h }, [
        node("rect", { width: w, height: h, fill: "white" }),
        node("ellipse", { cx: 0, cy: h, rx: canvas.clear.rx, ry: canvas.clear.ry, fill: "black" }),
      ]),
    ]),
    node("rect", { width: w, height: h, fill: ground.fill }),
    ...drawn,
    ...text,
  ]);
}

const KEEP_CAMEL = new Set(["viewBox", "maskUnits", "gradientUnits", "filterUnits", "baseFrequency", "numOctaves", "stitchTiles", "in2"]);
const attrName = (name: string) => (KEEP_CAMEL.has(name) ? name : name.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`));
const escapeXml = (text: string) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export function serializeArt(n: ArtNode): string {
  const attrs = Object.entries(n.attrs)
    .map(([k, v]) => ` ${attrName(k)}="${escapeXml(String(v))}"`)
    .join("");
  const inner = n.text !== undefined ? escapeXml(n.text) : (n.children ?? []).map(serializeArt).join("");
  return inner ? `<${n.tag}${attrs}>${inner}</${n.tag}>` : `<${n.tag}${attrs}/>`;
}
