import { expect, test, type Page } from "@playwright/test";
import { componentPages, examplePages, open, tokens } from "./support";

const pages = [...componentPages, ...examplePages];
const FIELDS =
  'input:not([type="checkbox"], [type="radio"], [type="range"], [type="color"], [type="file"], [type="button"], [type="submit"], [type="reset"], [type="image"], [type="hidden"]), textarea, select, [contenteditable]:not([contenteditable="false"])';

async function fieldSizes(page: Page) {
  return page.evaluate((selector) => {
    return [...document.querySelectorAll(selector)].map((el) => ({
      field: `${el.tagName.toLowerCase()} "${el.getAttribute("aria-label") ?? el.getAttribute("placeholder") ?? el.id ?? ""}"`,
      size: parseFloat(getComputedStyle(el).fontSize),
    }));
  }, FIELDS);
}

test.describe("no zoom when a field takes focus on a phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  for (const path of pages) {
    test(path, async ({ page }) => {
      await open(page, path);
      const small = (await fieldSizes(page)).filter((f) => f.size < 16).map((f) => `${f.field} is ${f.size}px`);
      expect(small).toEqual([]);
    });
  }

  test("the time's hours and minutes still fit their segments at the larger size", async ({ page }) => {
    await open(page, "/components/date-picker/examples");
    const segments = page.getByRole("textbox", { name: /^(Hours|Minutes)$/ });
    expect(await segments.count()).toBeGreaterThan(0);
    const clipped = await segments.evaluateAll((els) => els.filter((el) => el.scrollWidth > el.clientWidth).map((el) => el.getAttribute("aria-label")));
    expect(clipped).toEqual([]);
  });

  test("buttons and links take a quick second tap as a tap, not a zoom", async ({ page }) => {
    await open(page, "/components/button");
    const actions = await page.evaluate(() =>
      [...document.querySelectorAll("[data-preview] .pv-stage :is(button, a)")].map((el) => getComputedStyle(el).touchAction),
    );
    expect(actions.length).toBeGreaterThan(0);
    expect(new Set(actions)).toEqual(new Set(["manipulation"]));
  });
});

test("fields keep the body size with a mouse", async ({ page }) => {
  await open(page, "/components/input");
  const sizes = await fieldSizes(page);
  expect(sizes.length).toBeGreaterThan(0);
  expect(sizes.map((f) => f.size)).toContain(tokens.type["body"]?.size);
});
