import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const assets = join(root, "crates/kaeso-mail/assets");
const { art } = await import(join(root, "packages/tokens/src/tokens.ts"));
const { drawReleaseArt, serializeArt, RELEASE_ART_VIEW } = await import(join(root, "packages/ui/src/components/release-art/drawing.ts"));

export const COVERS = [
  { name: "release", art: { version: 1, seed: 2611, style: "bauhaus", palette: 0 } },
  { name: "welcome", art: { version: 1, seed: 1407, style: "arcs", palette: 2 } },
  { name: "summary", art: { version: 1, seed: 3301, style: "waves", palette: 6 } },
  { name: "costs", art: { version: 1, seed: 905, style: "albers", palette: 4 } },
  { name: "apps", art: { version: 1, seed: 7212, style: "truchet", palette: 1 } },
];

// The wordmark sits where the release art keeps its corner clear for a title, 40 units in.
const MARK = { x: 40, bottom: 40, width: 168 };

function wordmark(color) {
  const svg = readFileSync(join(assets, "wordmark-light.svg"), "utf8");
  const [, x, y, w] = /viewBox="([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+)"/.exec(svg).map(Number);
  const fill = svg.slice(svg.lastIndexOf("<g "));
  const paths = [...fill.matchAll(/<path d="([^"]+)"\/>/g)].map((m) => `<path d="${m[1]}"/>`).join("");
  const scale = MARK.width / w;
  const height = (MARK.width / 4) | 0;
  const { h } = RELEASE_ART_VIEW.wide;
  return `<g fill="${color}" transform="translate(${MARK.x} ${h - MARK.bottom - height}) scale(${scale.toFixed(6)}) translate(${-x} ${-y})">${paths}</g>`;
}

export function cover(spec) {
  const svg = serializeArt(drawReleaseArt(spec, { shape: "wide", idPrefix: "cover" }));
  const ground = art.grounds[art.palettes[spec.palette].ground];
  return `${svg.replace(/<\/svg>$/, `${wordmark(ground.text)}</svg>`)}\n`;
}

export const file = (name) => join(assets, `cover-${name}.svg`);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  for (const c of COVERS) writeFileSync(file(c.name), cover(c.art));
}
