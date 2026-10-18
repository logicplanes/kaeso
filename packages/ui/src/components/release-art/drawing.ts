import { art, font } from "@logicplanes/kaeso-tokens";

export const RELEASE_ART_STYLES = ["bauhaus", "albers", "stripes", "waves", "truchet", "arcs", "phyllotaxis", "spiro"] as const;
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

export const RELEASE_ART_VIEW = { banner: { w: 640, h: 360 }, wide: { w: 840, h: 360 } } as const;
export type ReleaseArtShape = keyof typeof RELEASE_ART_VIEW;

const U = 40;
const CLEAR = { rx: U * 7.5, ry: U * 7.5 };

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
    style: RELEASE_ART_STYLES[pick(RELEASE_ART_STYLES.length)] ?? "bauhaus",
    palette: pick(art.palettes.length),
  };
}

export function nextReleaseArt(previous: readonly ReleaseArtSpec[], next: () => number = Math.random): ReleaseArtSpec {
  const recent = previous.slice(-3);
  const styles = RELEASE_ART_STYLES.filter((s) => !recent.some((p) => p.style === s));
  const palettes = art.palettes.map((_, i) => i).filter((i) => !previous.slice(-4).some((p) => p.palette === i));
  const seeds = new Set(previous.map((p) => p.seed));
  const pick = <T>(list: readonly T[]) => list[Math.min(list.length - 1, Math.floor(next() * list.length))];
  let seed = Math.min(0xffffffff, Math.floor(next() * 0x100000000));
  while (seeds.has(seed)) seed = (seed + 1) >>> 0;
  return { version: 1, seed, style: pick(styles) ?? "bauhaus", palette: pick(palettes) ?? 0 };
}

export function releaseArtFor(text: string): ReleaseArtSpec {
  const seed = hash(text);
  return {
    version: 1,
    seed,
    style: RELEASE_ART_STYLES[seed % RELEASE_ART_STYLES.length] ?? "bauhaus",
    palette: (seed >>> 8) % art.palettes.length,
  };
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
  color: (i: number) => string;
  id: (name: string) => string;
}

const open = (c: Canvas, x: number, y: number) => (x / CLEAR.rx) ** 2 + ((c.h - y) / CLEAR.ry) ** 2 < 1;
const clearEdge = (c: Canvas, y: number) => {
  const dy = (c.h - y) / CLEAR.ry;
  return dy >= 1 ? 0 : CLEAR.rx * Math.sqrt(1 - dy * dy);
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
  const lo = CLEAR.rx - U / 2;
  const hi = c.w - margin - side;
  const x0 = lo + Math.round((c.next() * Math.max(0, hi - lo)) / (U / 2)) * (U / 2);
  const unit = side / 20;
  const levels = c.int(3, 4);
  const order = [0, 1, 2, 3].sort(() => c.next() - 0.5);
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
  const cx = U * c.int(Math.ceil((CLEAR.rx + r) / U), c.cols - 2);
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
    best = Math.min(best, Math.hypot(cx - CLEAR.rx * Math.cos(a), cy - (c.h - CLEAR.ry * Math.sin(a))));
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

const STYLE = { bauhaus, albers, stripes, waves, truchet, arcs, phyllotaxis, spiro } as const;

export interface DrawOptions {
  shape?: ReleaseArtShape | undefined;
  title?: string | undefined;
  eyebrow?: string | undefined;
  idPrefix?: string | undefined;
}

const paletteOf = (spec: ReleaseArtSpec) => art.palettes[spec.palette % art.palettes.length] ?? art.palettes[0];

export function releaseArtText(spec: ReleaseArtSpec) {
  return { color: art.grounds[paletteOf(spec).ground].text, eyebrow: { x: U, bottom: 96 }, title: { x: U - 2, bottom: U } };
}

export function drawReleaseArt(spec: ReleaseArtSpec, options: DrawOptions = {}): ArtNode {
  const { w, h } = RELEASE_ART_VIEW[options.shape ?? "banner"];
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
    color: (i) => colors[((i % colors.length) + colors.length) % colors.length] ?? ground.text,
    id: (name) => `${prefix}-${name}`,
  };
  const mask = canvas.id("clear");
  const drawn = STYLE[spec.style](canvas);
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
        node("ellipse", { cx: 0, cy: h, rx: CLEAR.rx, ry: CLEAR.ry, fill: "black" }),
      ]),
    ]),
    node("rect", { width: w, height: h, fill: ground.fill }),
    ...drawn,
    ...text,
  ]);
}

const KEEP_CAMEL = new Set(["viewBox", "maskUnits"]);
const attrName = (name: string) => (KEEP_CAMEL.has(name) ? name : name.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`));
const escapeXml = (text: string) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export function serializeArt(n: ArtNode): string {
  const attrs = Object.entries(n.attrs)
    .map(([k, v]) => ` ${attrName(k)}="${escapeXml(String(v))}"`)
    .join("");
  const inner = n.text !== undefined ? escapeXml(n.text) : (n.children ?? []).map(serializeArt).join("");
  return inner ? `<${n.tag}${attrs}>${inner}</${n.tag}>` : `<${n.tag}${attrs}/>`;
}
