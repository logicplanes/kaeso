import { expect, test } from "@playwright/test";
import { componentPages, examplePages, growToContent, open } from "./support";

const pages = [...componentPages, ...examplePages];

test.describe("phone width", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  for (const path of pages) {
    test(path, async ({ page }) => {
      await open(page, path);
      await growToContent(page, 390);
      const found = await page.evaluate(() => {
        const out: string[] = [];
        const overflow = document.documentElement.scrollWidth - innerWidth;
        if (overflow > 0) out.push(`the page scrolls sideways by ${overflow}px`);
        for (const pane of document.querySelectorAll("[data-preview]")) {
          const box = pane.getBoundingClientRect();
          for (const el of pane.querySelectorAll(".pv-stage *")) {
            const r = el.getBoundingClientRect();
            if (!r.width || !r.height) continue;
            const cs = getComputedStyle(el);
            if (el.tagName === "INPUT" && (cs.opacity === "0" || r.width <= 2)) continue;
            if (cs.position === "fixed") continue;
            let scroller = el.parentElement;
            let contained = false;
            while (scroller && scroller !== pane) {
              if (/(auto|scroll|hidden|clip)/.test(getComputedStyle(scroller).overflowX)) {
                contained = true;
                break;
              }
              scroller = scroller.parentElement;
            }
            if (contained) continue;
            if (r.right > box.right + 1 || r.left < box.left - 1) {
              out.push(
                `${pane.id || "preview"}: ${el.tagName.toLowerCase()} "${(el.textContent ?? "").trim().slice(0, 32)}" spans ${Math.round(r.left - box.left)}–${Math.round(r.right - box.left)} of ${Math.round(box.width)}px`,
              );
              break;
            }
          }
        }
        return out;
      });
      expect(found).toEqual([]);
    });
  }
});

for (const reducedMotion of ["no-preference", "reduce"] as const) {
  test.describe(`no resize loops with ${reducedMotion}`, () => {
    test.use({ reducedMotion });
    for (const path of pages) {
      test(path, async ({ page }) => {
        await page.addInitScript(() => {
          Object.assign(window, { resizeLoops: 0 });
          window.addEventListener("error", (event) => {
            if (String(event.message).includes("ResizeObserver loop")) {
              Object.assign(window, { resizeLoops: (window as unknown as { resizeLoops: number }).resizeLoops + 1 });
            }
          });
        });
        await open(page, path);
        for (const width of [1024, 390, 1440]) {
          await page.setViewportSize({ width, height: 900 });
          await page.waitForTimeout(120);
        }
        const triggers = page.locator("main [aria-haspopup='dialog']:visible");
        const count = Math.min(await triggers.count(), 6);
        for (let i = 0; i < count; i++) {
          await triggers
            .nth(i)
            .click({ timeout: 2000 })
            .catch(() => undefined);
          await page.keyboard.type("g");
          await page.waitForTimeout(400);
          await page.keyboard.press("Escape");
          await page.keyboard.press("Escape");
          await page.waitForTimeout(250);
        }
        expect(await page.evaluate(() => (window as unknown as { resizeLoops: number }).resizeLoops)).toBe(0);
      });
    }
  });
}

test("the home page counts the components the sidebar lists, each with its own page", async ({ page }) => {
  await open(page, "/");
  const listed = page.locator('nav[data-sidebar-root] a[href^="/components/"]');
  await expect(page.locator("main h1")).toContainText(String(componentPages.length));
  await expect(listed).toHaveCount(componentPages.length);
});

test("table rows keep their padding when text and links wrap, and one-line rows stay 44px", async ({ page }) => {
  await open(page, "/components/table/examples");
  const rows = await page.locator("main table tbody tr").evaluateAll((trs) =>
    trs.map((tr) => {
      const cells = [...tr.querySelectorAll("td")];
      return {
        height: tr.getBoundingClientRect().height,
        wraps: cells.some((td) => {
          const range = document.createRange();
          range.selectNodeContents(td);
          return range.getBoundingClientRect().height > parseFloat(getComputedStyle(td).lineHeight) * 1.5;
        }),
        padding: cells.map((td) => {
          const range = document.createRange();
          range.selectNodeContents(td);
          const text = range.getBoundingClientRect();
          const box = td.getBoundingClientRect();
          return Math.min(text.top - box.top, box.bottom - text.bottom);
        }),
      };
    }),
  );
  expect(rows.some((r) => r.wraps)).toBe(true);
  for (const row of rows) {
    if (!row.wraps) expect(row.height).toBe(44);
    for (const padding of row.padding) expect(padding).toBeGreaterThanOrEqual(10);
  }
  const links = await page
    .locator("main table a")
    .evaluateAll((as) => as.map((a) => ({ display: getComputedStyle(a).display, fragments: a.getClientRects().length })));
  expect(links.every((l) => l.display === "inline")).toBe(true);
  expect(links.some((l) => l.fragments > 1)).toBe(true);
});
