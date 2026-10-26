import { expect, test } from "@playwright/test";
import { open, statesPages } from "./support";

test.use({ viewport: { width: 1600, height: 1200 } });

// Identical states that later pull requests in this stack fix; each one removes its own entries.
const PENDING = new Set([
  "/components/button-group/states: dark attached: hover paints like rest",
  ...["light", "dark"].flatMap((theme) => [
    ...["unchecked", "checked", "indeterminate", "invalid"].map((row) => `/components/checkbox/states: ${theme} ${row}: pressed paints like rest`),
    `/components/checkbox/states: ${theme} invalid: hover paints like rest`,
    `/components/model-picker/states: ${theme} raised: pressed paints like rest`,
    ...["unselected", "selected"].map((row) => `/components/radio/states: ${theme} ${row}: pressed paints like rest`),
    `/components/radio/states: ${theme} selected: hover paints like rest`,
    `/components/switch/states: ${theme} on: hover paints like rest`,
  ]),
  "/components/tag/states: dark raised: hover paints like rest",
]);

test.describe("every forced state paints differently from rest", () => {
  for (const path of statesPages) {
    test(path, async ({ page }) => {
      await open(page, path);
      const same = await page.evaluate(() => {
        const STATES = ["hover", "pressed", "selected", "checked", "disabled", "open"];
        const STEP = 3;
        const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
        if (!ctx) throw new Error("no 2d canvas");
        const rgba = (colour: string) => {
          ctx.clearRect(0, 0, 1, 1);
          ctx.fillStyle = "#000";
          ctx.fillStyle = colour;
          ctx.fillRect(0, 0, 1, 1);
          const d = ctx.getImageData(0, 0, 1, 1).data;
          return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0, (d[3] ?? 0) / 255] as const;
        };
        const linear = (c: number) => (c / 255 <= 0.04045 ? c / 255 / 12.92 : ((c / 255 + 0.055) / 1.055) ** 2.4);
        const lightness = ([r, g, b]: readonly number[]) => {
          const y = 0.2126 * linear(r ?? 0) + 0.7152 * linear(g ?? 0) + 0.0722 * linear(b ?? 0);
          return y > 216 / 24389 ? 116 * Math.cbrt(y) - 16 : (24389 / 27) * y;
        };
        const surface = (el: Element | null): readonly number[] => {
          for (let e = el; e; e = e.parentElement) {
            const c = rgba(getComputedStyle(e).backgroundColor);
            if (c[3] > 0) return c;
          }
          return [255, 255, 255, 1];
        };
        const over = (fg: readonly number[], bg: readonly number[]) => [0, 1, 2].map((i) => (fg[i] ?? 0) * (fg[3] ?? 1) + (bg[i] ?? 0) * (1 - (fg[3] ?? 1)));
        const brightness = (c: number[], filter: string) => {
          const k = Number(/brightness\(([\d.]+)\)/.exec(filter)?.[1] ?? 1);
          return c.map((v) => Math.min(255, v * k));
        };
        const paint = (el: Element) => {
          const cs = getComputedStyle(el);
          const under = surface(el.parentElement);
          const opacity = Number(cs.opacity);
          const fade = (c: number[]) => c.map((v, i) => v * opacity + (under[i] ?? 0) * (1 - opacity));
          const fill = brightness(over(rgba(cs.backgroundColor), under), cs.filter);
          return {
            fill: lightness(fade(fill)),
            text: lightness(fade(brightness(over(rgba(cs.color), fill), cs.filter))),
            marks: [cs.boxShadow, cs.transform, cs.textDecorationColor, cs.borderColor].join("|"),
          };
        };
        const tree = (el: Element) => [el, ...el.querySelectorAll("*")].filter((e) => !(e instanceof SVGElement) || e instanceof SVGSVGElement);
        const found: string[] = [];
        for (const theme of ["light", "dark"]) {
          const section = document.querySelector(`[data-states-theme="${theme}"]`);
          for (const rest of section?.querySelectorAll<HTMLElement>('[data-cell$="-rest"]') ?? []) {
            const row = (rest.dataset.cell ?? "").slice(0, -"-rest".length);
            const a = rest.firstElementChild;
            if (!a) continue;
            for (const state of STATES) {
              const b = section?.querySelector(`[data-cell="${row}-${state}"]`)?.firstElementChild;
              if (!b) continue;
              const as = tree(a);
              const bs = tree(b);
              const differs =
                as.length !== bs.length ||
                as.some((el, i) => {
                  const other = bs[i];
                  if (!other) return true;
                  const p = paint(el);
                  const q = paint(other);
                  return Math.abs(p.fill - q.fill) >= STEP || Math.abs(p.text - q.text) >= STEP || p.marks !== q.marks;
                });
              if (!differs) found.push(`${theme} ${row}: ${state} paints like rest`);
            }
          }
        }
        return found;
      });
      expect(same.filter((line) => !PENDING.has(`${path}: ${line}`))).toEqual([]);
    });
  }
});
