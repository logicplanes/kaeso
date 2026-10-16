import { expect, test } from "@playwright/test";
import { open, statesPages, tokens } from "./support";

const SIZES = [...new Set(Object.values(tokens.type).map((s) => `${s.size}px`))];
const LEADING = [...new Set([...Object.values(tokens.type).map((s) => `${s.lineHeight}px`), "20px", "normal"])];
const WEIGHTS = ["400", "500", "600", "480", "580"];
const DURATIONS = [0, ...Object.values(tokens.motion.duration), tokens.motion.skeleton.duration, tokens.motion.spinner.turn];

for (const theme of ["light", "dark"] as const) {
  test.describe(`type scale in ${theme}`, () => {
    test.use({ colorScheme: theme });
    for (const path of statesPages) {
      test(path, async ({ page }) => {
        await open(page, path);
        const found = await page.evaluate(
          ({ sizes, leading, weights }) => {
            const out: string[] = [];
            const scope = document.querySelector("main") ?? document.body;
            for (const el of scope.querySelectorAll("*")) {
              if (![...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent?.trim())) continue;
              if (el.closest("pre, svg, [data-state-label], .prose-k")) continue;
              const cs = getComputedStyle(el);
              const tag = `${el.tagName.toLowerCase()} "${(el.textContent ?? "").trim().slice(0, 20)}"`;
              if (!sizes.includes(cs.fontSize)) out.push(`font-size ${cs.fontSize}: ${tag}`);
              if (!weights.includes(cs.fontWeight)) out.push(`font-weight ${cs.fontWeight}: ${tag}`);
              if (!leading.includes(cs.lineHeight) && cs.lineHeight !== cs.fontSize) out.push(`line-height ${cs.lineHeight}: ${tag}`);
              if (/serif/.test(cs.fontFamily) && !/sans-serif/.test(cs.fontFamily)) out.push(`serif font: ${tag}`);
            }
            return [...new Set(out)];
          },
          { sizes: SIZES, leading: LEADING, weights: WEIGHTS },
        );
        expect(found).toEqual([]);
      });
    }
  });
}

for (const reducedMotion of ["no-preference", "reduce"] as const) {
  test.describe(`motion with ${reducedMotion}`, () => {
    test.use({ reducedMotion });
    for (const path of statesPages) {
      test(path, async ({ page }) => {
        await open(page, path);
        const found = await page.evaluate(
          ({ allowed, reduced }) => {
            const out: string[] = [];
            for (const el of document.querySelectorAll("body *")) {
              const cs = getComputedStyle(el);
              const name = `${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)}`;
              const properties = cs.transitionProperty.split(",").map((s) => s.trim());
              const durations = cs.transitionDuration.split(",").map((s) => parseFloat(s) * (s.trim().endsWith("ms") ? 1 : 1000));
              if (properties.includes("all") && durations.some((d) => d > 0)) out.push(`transition: all on ${name}`);
              for (const d of durations) if (!allowed.includes(Math.round(d))) out.push(`duration ${d}ms on ${name}`);
              if (reduced && cs.animationName !== "none" && /(^|,\s*)k-(enter|in|scale|slide|pop)(-|$|,)/.test(cs.animationName)) {
                out.push(`animation ${cs.animationName} under reduced motion`);
              }
            }
            return [...new Set(out)];
          },
          { allowed: DURATIONS, reduced: reducedMotion === "reduce" },
        );
        expect(found).toEqual([]);
      });
    }
  });
}

test.describe("geometry", () => {
  for (const path of statesPages) {
    test(path, async ({ page }) => {
      await open(page, path);
      const found = await page.evaluate(() => {
        const out: string[] = [];
        const label = (el: Element) =>
          `${el.tagName.toLowerCase()}${el.getAttribute("aria-label") ? `[${el.getAttribute("aria-label")}]` : ""} "${(el.textContent ?? "").trim().slice(0, 24)}"`;
        const controls = [
          ...document.querySelectorAll<HTMLElement>(
            'button, a[class*="rounded"], input:not([type=hidden]):not([type=checkbox]):not([type=radio]), [role=switch], [role=tab], [role=menuitem], [role=option], [role=combobox], [data-row]',
          ),
        ].filter((el) => el.offsetParent !== null && !el.closest("[data-state-label], pre, .prose-k, [aria-hidden=true]"));
        for (const el of controls) {
          const rect = el.getBoundingClientRect();
          if (rect.width === 0 || rect.height === 0) continue;
          const cs = getComputedStyle(el);
          if (cs.display === "contents" || (cs.position === "absolute" && rect.height < 12)) continue;
          const h = Math.round((cs.boxSizing === "border-box" ? parseFloat(cs.height) : rect.height) * 100) / 100;
          if (Math.abs(h - Math.round(h)) > 0.01) out.push(`fractional height ${h}px: ${label(el)}`);
          const svgs = el.querySelectorAll(":scope > svg, :scope > span > svg");
          const icon = svgs[0];
          if (svgs.length === 1 && icon && !(el.textContent ?? "").trim()) {
            const s = icon.getBoundingClientRect();
            const dx = Math.abs(s.left + s.width / 2 - (rect.left + rect.width / 2));
            const dy = Math.abs(s.top + s.height / 2 - (rect.top + rect.height / 2));
            if (dx > 0.5 || dy > 0.5) out.push(`icon off center by ${dx.toFixed(1)},${dy.toFixed(1)}: ${label(el)}`);
          }
        }
        for (const el of document.querySelectorAll<HTMLInputElement>(
          "input:not([type]), input[type=text], input[type=search], input[type=email], input[type=url], input[type=number], input[type=password], input[type=tel]",
        )) {
          if (el.offsetParent === null || el.closest("[aria-hidden=true]")) continue;
          const cs = getComputedStyle(el);
          const lineHeight = parseFloat(cs.lineHeight);
          const parent = el.parentElement;
          if (!parent || !Number.isFinite(lineHeight) || cs.opacity === "0") continue;
          const ps = getComputedStyle(parent);
          if (!/flex/.test(ps.display) || parseFloat(ps.borderTopLeftRadius) < 8) continue;
          const box = parseFloat(cs.height) - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
          if (box > lineHeight + 0.5) out.push(`input ${box}px tall in a ${lineHeight}px line, text off the baseline: ${label(el)}`);
        }
        for (const el of document.querySelectorAll<HTMLElement>("button, a, td, th, span, p, label")) {
          if (el.children.length || el.offsetParent === null) continue;
          const cs = getComputedStyle(el);
          if (el.clientWidth <= 2 || cs.clip !== "auto" || cs.clipPath !== "none") continue;
          if (el.scrollWidth > el.clientWidth + 1 && cs.overflow !== "visible" && cs.textOverflow !== "ellipsis") {
            out.push(`clipped text without ellipsis: ${label(el)}`);
          }
        }
        return [...new Set(out)];
      });
      expect(found).toEqual([]);
    });
  }
});
