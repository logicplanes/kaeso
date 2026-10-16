import { describe, expect, it } from "vitest";
import { tokensCss } from "./tokens-css.ts";

type Root = { theme?: string | undefined };
type Media = { dark: boolean; contrast: boolean };
type Rule = { media: string | undefined; selectors: string[]; declarations: Map<string, string>; order: number };

const css = tokensCss({
  fixed: ["  --k-radius-pill: 9999px;"],
  light: ["  --k-text: #111111;", "  --k-text-tertiary: #888888;"],
  dark: ["  --k-text: #eeeeee;", "  --k-text-tertiary: #777777;"],
});

function parse(text: string): Rule[] {
  const rules: Rule[] = [];
  for (const m of text.matchAll(/(?:@media \(([^)]+)\)\s*\{\s*)?([^{}@]+)\{([^{}]*)\}/g)) {
    const declarations = new Map<string, string>();
    for (const d of (m[3] ?? "").matchAll(/(--[\w-]+):\s*([^;]+);/g)) declarations.set(d[1] ?? "", (d[2] ?? "").trim());
    rules.push({ media: m[1], selectors: (m[2] ?? "").split(",").map((s) => s.trim()), declarations, order: rules.length });
  }
  return rules;
}

function parts(selector: string): string[] {
  return selector.match(/:not\([^)]*\)|:root|\[[^\]]+\]/g) ?? [];
}

function specificity(selector: string): number {
  return parts(selector).reduce((n, p) => n + (p.startsWith(":not(") ? specificity(p.slice(5, -1)) : 1), 0);
}

function matches(selector: string, root: Root): boolean {
  return parts(selector).every((p) => {
    if (p === ":root") return true;
    if (p.startsWith(":not(")) return !matches(p.slice(5, -1), root);
    const attr = /^\[data-theme(?:="([^"]+)")?\]$/.exec(p);
    return attr !== null && root.theme !== undefined && (attr[1] === undefined || attr[1] === root.theme);
  });
}

function applies(media: string | undefined, env: Media): boolean {
  if (media === undefined) return true;
  if (media === "prefers-color-scheme: dark") return env.dark;
  if (media === "prefers-contrast: more") return env.contrast;
  throw new Error(`unknown media ${media}`);
}

function resolve(property: string, root: Root, env: Media): string | undefined {
  let best: { value: string; specificity: number; order: number } | undefined;
  for (const rule of parse(css)) {
    const value = rule.declarations.get(property);
    if (value === undefined || !applies(rule.media, env)) continue;
    const hits = rule.selectors.filter((s) => matches(s, root)).map(specificity);
    if (hits.length === 0) continue;
    const candidate = { value, specificity: Math.max(...hits), order: rule.order };
    if (!best || candidate.specificity > best.specificity || (candidate.specificity === best.specificity && candidate.order > best.order)) best = candidate;
  }
  return best?.value;
}

const showsDark = (root: Root, systemDark: boolean) => root.theme === "dark" || (systemDark && root.theme !== "light");

const roots: [string, Root][] = [
  ["no theme attribute", {}],
  ["the light theme", { theme: "light" }],
  ["the dark theme", { theme: "dark" }],
];

describe("tokens.css", () => {
  it.each(roots)("follows the system colour scheme with %s", (_, root) => {
    for (const dark of [false, true]) {
      expect(resolve("--k-text-tertiary", root, { dark, contrast: false })).toBe(showsDark(root, dark) ? "#777777" : "#888888");
    }
  });

  it.each(roots)("raises tertiary text to full contrast in light and dark with %s", (_, root) => {
    for (const dark of [false, true]) {
      expect(resolve("--k-text-tertiary", root, { dark, contrast: true })).toBe("var(--k-text)");
      expect(resolve("--k-line-subtle", root, { dark, contrast: true })).toBe("var(--k-line-strong)");
      expect(resolve("--k-text", root, { dark, contrast: true })).toBe(showsDark(root, dark) ? "#eeeeee" : "#111111");
    }
  });

  it("keeps the values that do not change between modes", () => {
    expect(resolve("--k-radius-pill", {}, { dark: true, contrast: true })).toBe("9999px");
  });
});
