import { expect, test } from "@playwright/test";
import { open } from "./support";

test("Next stays under the pointer while pages change, and keeps focus on the last page", async ({ page }) => {
  await open(page, "/components/pagination/examples");
  const pages = page.getByRole("navigation", { name: "Pagination" }).filter({ has: page.getByRole("button", { name: "Page 12" }) });
  const next = pages.getByRole("button", { name: "Next page" });
  await expect(pages.getByRole("button", { name: "Page 4" })).toHaveAttribute("aria-current", "page");
  const xs = new Set<number>();
  for (let i = 0; i < 7; i++) {
    xs.add(Math.round((await next.boundingBox())!.x));
    await next.click();
  }
  expect([...xs]).toHaveLength(1);
  await next.focus();
  await page.keyboard.press("Enter");
  await expect(pages.getByRole("button", { name: "Page 12" })).toHaveAttribute("aria-current", "page");
  await page.keyboard.press("Enter");
  await expect(next).toBeFocused();
  await expect(next).toHaveAttribute("aria-disabled", "true");
});

test("underline tabs stay inside their column, so no label or ring reaches the gutter", async ({ page }) => {
  await open(page, "/components/tabs/states");
  const list = page.locator('[data-states-theme="light"] [role=tablist]').filter({ hasText: "Antwortzeiten" });
  const column = await list.evaluate((el) => {
    const r = el.getBoundingClientRect();
    const p = el.parentElement!.getBoundingClientRect();
    return { left: r.left - p.left, right: p.right - r.right };
  });
  expect(column.left).toBeGreaterThanOrEqual(0);
  expect(column.right).toBeGreaterThanOrEqual(0);
});

test.describe("with a coarse pointer", () => {
  test.use({ hasTouch: true, isMobile: true, viewport: { width: 390, height: 844 } });
  test("navigation rows grow to 44px", async ({ page }) => {
    await open(page, "/components/sidebar/states");
    const heights = await page
      .locator('[data-states-theme="light"] [data-sidebar-root] li > a, [data-states-theme="light"] [data-sidebar-root] li > button')
      .evaluateAll((els) => els.slice(0, 6).map((el) => Math.round(el.getBoundingClientRect().height)));
    expect(heights.length).toBeGreaterThan(0);
    for (const h of heights) expect(h).toBe(44);
  });
});
