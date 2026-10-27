import { expect, test } from "@playwright/test";
import { open } from "./support";

test.use({ viewport: { width: 1280, height: 900 } });

test("a click in the composer's button row puts the cursor in the text box", async ({ page }) => {
  await open(page, "/components/composer/examples");
  const composer = page.locator("[data-preview] form").first();
  const box = composer.getByRole("textbox");
  const row = await composer.evaluate((form) => form.lastElementChild!.getBoundingClientRect().toJSON() as DOMRect);
  await page.mouse.click(row.left + row.width / 2, row.top + row.height / 2);
  await expect(box).toBeFocused();
});

test("Send and Stop keep the focus through a whole response", async ({ page }) => {
  await open(page, "/components/composer/examples");
  const composer = page.locator("[data-preview] form").first();
  await composer.getByRole("textbox").fill("Summarize the incident");
  await composer.getByRole("button", { name: "Send" }).focus();
  await page.keyboard.press("Enter");
  await expect(composer.getByRole("button", { name: "Stop" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(composer.getByRole("button", { name: "Send" })).toBeFocused();
});

test("a response keeps its text, footer and metrics on one line, code and tables on the edge", async ({ page }) => {
  await open(page, "/components/response-view/states");
  const light = page.locator('[data-states-theme="light"]');
  const withFooter = light.locator("[data-matrix='Done, no reasoning, with metrics'] article");
  const textLeft = (selector: string, scope = withFooter) =>
    scope
      .locator(selector)
      .first()
      .evaluate((el) => {
        const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, (n) => (n.textContent?.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP));
        const range = document.createRange();
        range.selectNodeContents(walker.nextNode()!);
        return Math.round(range.getBoundingClientRect().left);
      });
  const boxLeft = async (selector: string, scope = withFooter) => Math.round((await scope.locator(selector).first().boundingBox())!.x);
  expect(await textLeft("dl")).toBe(await textLeft("p"));
  const mixed = light.locator("[data-matrix='A table and code'] article");
  const edge = Math.round((await mixed.boundingBox())!.x);
  expect(await boxLeft("table", mixed)).toBe(edge);
  expect(await boxLeft(".rounded-well", mixed)).toBe(edge);
  expect(await textLeft("p", mixed)).toBe(edge + 12);
});

test("columns that don't fit are reached with Previous and Next, and a focused thread shows its whole ring", async ({ page }) => {
  await open(page, "/components/compare-grid/states");
  const grid = page.locator('[data-states-theme="light"] [data-matrix="More columns than fit: Previous and Next, faded edges"]');
  const next = grid.getByRole("button", { name: "Next column" });
  await expect(next).toBeVisible();
  await expect(grid.getByRole("button", { name: "Previous column" })).toBeDisabled();
  await next.click();
  await expect(grid.getByRole("button", { name: "Previous column" })).toBeEnabled();

  const two = page.locator('[data-states-theme="light"] [data-matrix="One column"]');
  await expect(two.getByRole("button", { name: "Next column" })).toHaveCount(0);
  const thread = two.getByRole("region", { name: "GPT-5 thread" });
  await thread.focus();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Shift+Tab");
  await expect(thread).toBeFocused();
  const ring = await thread.evaluate((el) => {
    const after = getComputedStyle(el.parentElement!, "::after");
    return { color: after.outlineColor, width: after.outlineWidth, mask: getComputedStyle(el.parentElement!).maskImage };
  });
  expect(ring.width).toBe("2px");
  expect(ring.color).not.toBe("rgba(0, 0, 0, 0)");
  expect(ring.mask).toBe("none");
});
