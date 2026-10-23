import { describe, expect, it } from "vitest";
import { dangerFill, neutral, semantic, shadow, viz } from "./tokens.js";

type Rgb = readonly [number, number, number];
type Mode = "light" | "dark";
const MODES: readonly Mode[] = ["light", "dark"];

function rgb(hex: string): Rgb | undefined {
  const match = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!match?.[1]) return undefined;
  const value = match[1];
  return [0, 2, 4].map((i) => parseInt(value.slice(i, i + 2), 16) / 255) as unknown as Rgb;
}

function luminance([r, g, b]: Rgb): number {
  const linear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

function contrast(a: Rgb, b: Rgb): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (high + 0.05) / (low + 0.05);
}

function ratio(foreground: string, background: string): number {
  const fg = rgb(foreground);
  const bg = rgb(background);
  if (!fg || !bg) throw new Error(`${foreground} or ${background} is not a #rrggbb colour`);
  return contrast(fg, bg);
}

const surfaces = ["bg-panel", "bg-chrome", "bg-raised", "fill-1", "fill-2"] as const;
const neutralRules: readonly (readonly [keyof typeof neutral, keyof typeof neutral, number])[] = [
  ...(["text", "text-secondary"] as const).flatMap((fg) => surfaces.map((bg) => [fg, bg, 4.5] as const)),
  ...surfaces.filter((s) => s !== "fill-2").map((bg) => ["text-tertiary", bg, 4.5] as const),
  ...(["text", "text-secondary"] as const).flatMap((fg) => (["item-hover", "item-pressed"] as const).map((bg) => [fg, bg, 4.5] as const)),
  ["on-ink", "ink", 4.5],
  ["toast-fg", "toast-bg", 4.5],
  ["toast-fg-secondary", "toast-bg", 4.5],
  ["focus", "bg-panel", 3],
  ["line-strong", "bg-panel", 1.3],
  ...(["line-field", "line-field-hover"] as const).flatMap((edge) => surfaces.map((bg) => [edge, bg, 3] as const)),
  ...(["item-hover", "item-pressed"] as const).map((row) => ["line-field", row, 3] as const),
];
const darkSwitchThumbRules: readonly (readonly [keyof typeof neutral, keyof typeof neutral, number])[] = (
  ["fill-3", "line-strong", "bg-panel", "bg-raised"] as const
).map((bg) => ["switch-thumb", bg, 3] as const);

describe.each(MODES)("contrast in %s mode", (mode) => {
  it.each(neutralRules)("%s on %s reaches %d:1", (fg, bg, min) => {
    expect(ratio(neutral[fg][mode], neutral[bg][mode])).toBeGreaterThanOrEqual(min);
  });

  it.each(Object.keys(semantic) as (keyof typeof semantic)[])("%s text is readable on every surface and its tint", (role) => {
    const text = semantic[role].text[mode];
    for (const bg of ["bg-panel", "bg-raised", "fill-1", "fill-2"] as const) {
      expect(ratio(text, neutral[bg][mode]), `${role} on ${bg}`).toBeGreaterThanOrEqual(4.5);
    }
    const tint = rgb(semantic[role].tint[mode]);
    if (tint) expect(contrast(rgb(text) ?? tint, tint)).toBeGreaterThanOrEqual(4.5);
  });

  it("white reads on the danger fill", () => {
    expect(ratio("#ffffff", dangerFill[mode])).toBeGreaterThanOrEqual(4.5);
  });

  it.each(viz.map((v) => [v.name, v] as const))("chart colour %s stands out from the panel", (_, colour) => {
    expect(ratio(mode === "light" ? colour.thin : colour.dark, neutral["bg-panel"][mode])).toBeGreaterThanOrEqual(3);
  });
});

it.each(darkSwitchThumbRules)("in dark mode the off switch's %s stands out from %s by %d:1", (fg, bg, min) => {
  expect(ratio(neutral[fg].dark, neutral[bg].dark)).toBeGreaterThanOrEqual(min);
});

it("draws select and date triggers with the same ring as text inputs", () => {
  expect(shadow.selector).toEqual(shadow.field);
  expect(shadow["selector-hover"]).toEqual(shadow["field-hover"]);
});

it("refuses colours that are not #rrggbb", () => {
  expect(() => ratio("rgb(0 0 0)", "#ffffff")).toThrow("not a #rrggbb colour");
});
