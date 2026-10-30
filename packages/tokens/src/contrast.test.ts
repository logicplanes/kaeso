import { describe, expect, it } from "vitest";
import { dangerFill, diff, neutral, providerMark, semantic, shadow, viz } from "./tokens.js";

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

function lightness(hex: string): number {
  const colour = rgb(hex);
  if (!colour) throw new Error(`${hex} is not a #rrggbb colour`);
  const y = luminance(colour);
  return y > 216 / 24389 ? 116 * Math.cbrt(y) - 16 : (24389 / 27) * y;
}

function apca(text: string, background: string): number {
  const screen = (hex: string) => {
    const colour = rgb(hex);
    if (!colour) throw new Error(`${hex} is not a #rrggbb colour`);
    const [r, g, b] = colour.map((c) => c ** 2.4) as unknown as Rgb;
    const y = 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
    return y > 0.022 ? y : y + (0.022 - y) ** 1.414;
  };
  const [t, b] = [screen(text), screen(background)];
  const contrast = b > t ? (b ** 0.56 - t ** 0.57) * 1.14 : (b ** 0.65 - t ** 0.62) * 1.14;
  return Math.abs(contrast) < 0.1 ? 0 : Math.abs(contrast) * 100 - 2.7;
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
  ...surfaces.map((bg) => ["text-tertiary", bg, 4.5] as const),
  ...(["text", "text-secondary"] as const).flatMap((fg) => (["item-hover", "item-pressed"] as const).map((bg) => [fg, bg, 4.5] as const)),
  ...(["ink", "ink-hover", "ink-pressed"] as const).map((bg) => ["on-ink", bg, 4.5] as const),
  ...(["text", "text-secondary"] as const).flatMap((fg) => (["raised-hover", "raised-pressed"] as const).map((bg) => [fg, bg, 4.5] as const)),
  ["toast-fg", "toast-bg", 4.5],
  ["toast-fg-secondary", "toast-bg", 4.5],
  ["focus", "bg-panel", 3],
  ["line-strong", "bg-panel", 1.3],
  ...(["line-field", "line-field-hover"] as const).flatMap((edge) => surfaces.map((bg) => [edge, bg, 3] as const)),
  ...(["item-hover", "item-pressed"] as const).map((row) => ["line-field", row, 3] as const),
];
const stateSteps: readonly (readonly [keyof typeof neutral, keyof typeof neutral])[] = [
  ["ink", "ink-hover"],
  ["ink-hover", "ink-pressed"],
  ["bg-raised", "raised-hover"],
  ["raised-hover", "raised-pressed"],
  ["bg-panel", "fill-1"],
  ["fill-1", "fill-2"],
  ["fill-2", "fill-3"],
];
const darkSwitchThumbRules: readonly (readonly [keyof typeof neutral, keyof typeof neutral, number])[] = (
  ["fill-3", "line-strong", "bg-panel", "bg-raised"] as const
).map((bg) => ["switch-thumb", bg, 3] as const);

describe.each(MODES)("contrast in %s mode", (mode) => {
  it.each(neutralRules)("%s on %s reaches %d:1", (fg, bg, min) => {
    expect(ratio(neutral[fg][mode], neutral[bg][mode])).toBeGreaterThanOrEqual(min);
  });

  it.each([
    ["text-secondary", 60],
    ["text-tertiary", 45],
  ] as const)("%s reaches APCA Lc %d on the panel, so small text never feels faint", (role, min) => {
    expect(apca(neutral[role][mode], neutral["bg-panel"][mode])).toBeGreaterThanOrEqual(min);
  });

  it.each(stateSteps)("%s and %s are at least 3 L* apart, so the state change is visible", (from, to) => {
    expect(Math.abs(lightness(neutral[from][mode]) - lightness(neutral[to][mode]))).toBeGreaterThanOrEqual(3);
  });

  it.each(Object.keys(semantic) as (keyof typeof semantic)[])("%s text is readable on every surface and its tint", (role) => {
    const text = semantic[role].text[mode];
    for (const bg of ["bg-panel", "bg-raised", "fill-1", "fill-2"] as const) {
      expect(ratio(text, neutral[bg][mode]), `${role} on ${bg}`).toBeGreaterThanOrEqual(4.5);
    }
    const tint = rgb(semantic[role].tint[mode]);
    if (tint) expect(contrast(rgb(text) ?? tint, tint)).toBeGreaterThanOrEqual(4.5);
  });

  it.each([
    ["added", "success"],
    ["removed", "danger"],
  ] as const)("code on a %s diff line and its changed words stays readable", (kind, role) => {
    const { line, word } = diff[kind];
    const code = [neutral.text[mode], semantic.info.text[mode], semantic.warning.text[mode]];
    for (const fg of [...code, neutral["text-secondary"][mode], neutral["text-tertiary"][mode], semantic[role].text[mode]]) {
      expect(ratio(fg, line[mode]), `${fg} on the ${kind} line`).toBeGreaterThanOrEqual(4.5);
    }
    for (const fg of code) expect(ratio(fg, word[mode]), `${fg} on ${kind} words`).toBeGreaterThanOrEqual(4.5);
    expect(ratio(word[mode], line[mode]), `${kind} words stand out from their line`).toBeGreaterThanOrEqual(1.25);
  });

  it("white reads on the danger fill", () => {
    expect(ratio("#ffffff", dangerFill[mode])).toBeGreaterThanOrEqual(4.5);
  });

  it.each(viz.map((v) => [v.name, v] as const))("chart colour %s stands out from the panel", (_, colour) => {
    expect(ratio(colour[mode], neutral["bg-panel"][mode])).toBeGreaterThanOrEqual(3);
    expect(ratio(colour[mode], neutral["fill-1"][mode])).toBeGreaterThanOrEqual(3);
  });
});

// Mistral's brand page forbids recolouring its icon, so its colours stay exact: 3:1 on the raised background in dark mode only.
const recoloured = Object.entries(providerMark).filter(([token]) => !token.startsWith("mistral-"));
const mistral = Object.entries(providerMark).filter(([token]) => token.startsWith("mistral-"));

it.each(recoloured)("provider mark colour %s stands out from white in light mode and from its tile in dark mode", (_, colour) => {
  expect(ratio(colour.light, neutral["bg-raised"].light)).toBeGreaterThanOrEqual(3);
  expect(ratio(colour.dark, neutral["fill-1"].dark)).toBeGreaterThanOrEqual(3);
});

it.each(mistral)("Mistral's colour %s stands out from the raised background in dark mode", (_, colour) => {
  expect(ratio(colour.dark, neutral["bg-raised"].dark)).toBeGreaterThanOrEqual(3);
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
