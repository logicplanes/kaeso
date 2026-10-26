import { expect, test } from "@playwright/test";
import { open } from "./support";

test.describe("overlays on a 320px screen", () => {
  test.use({ viewport: { width: 320, height: 568 } });

  test("a dialog's footer buttons share one row inside the dialog", async ({ page }) => {
    await open(page, "/components/dialog/examples");
    await page.locator("#ex-form").getByRole("button", { name: "Create key" }).first().click();
    const dialog = page.getByRole("dialog", { name: "Create key" });
    await expect(dialog).toBeVisible();
    const box = (await dialog.boundingBox())!;
    const cancel = (await dialog.getByRole("button", { name: "Cancel" }).boundingBox())!;
    const create = (await dialog.getByRole("button", { name: "Create key" }).boundingBox())!;
    expect(cancel.y).toBe(create.y);
    expect(cancel.x).toBeCloseTo(box.x + 24, 0);
    expect(create.x + create.width).toBeCloseTo(box.x + box.width - 24, 0);
  });

  test("a popover and a hover card stay on the screen", async ({ page }) => {
    await open(page, "/components/popover/examples");
    await page.locator("#ex-form").getByRole("button", { name: "Rename" }).click();
    const popover = (await page.getByRole("dialog", { name: "Rename key" }).boundingBox())!;
    expect(popover.x).toBeGreaterThanOrEqual(8);
    expect(popover.x + popover.width).toBeLessThanOrEqual(312);
    await page.keyboard.press("Escape");
    await page.locator("#ex-hover").getByRole("link").focus();
    const card = page.locator("[data-open].w-320");
    await expect(card).toBeVisible();
    const box = (await card.boundingBox())!;
    expect(box.x + box.width).toBeLessThanOrEqual(312);
  });
});

test("a single-line tooltip too long for the screen wraps inside it", async ({ page }) => {
  await page.setViewportSize({ width: 200, height: 600 });
  await open(page, "/components/tooltip/examples");
  const tip = page.getByRole("tooltip").filter({ hasText: "Go to request logs" });
  await expect(tip).toBeVisible();
  const box = (await tip.boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(8);
  expect(box.x + box.width).toBeLessThanOrEqual(192);
  expect(box.height).toBeGreaterThan(28);
});

test("a layer dialog's title keeps its place when the second layer opens", async ({ page }) => {
  await open(page, "/components/layer-dialog/examples");
  await page.getByRole("button", { name: "Add model" }).first().click();
  const title = page.getByRole("dialog").getByRole("heading").first();
  await expect(title).toHaveText("Add model");
  const before = (await title.boundingBox())!;
  await page.getByRole("dialog").getByRole("button", { name: "Add a custom model" }).click();
  await expect(title).toHaveText("Add a custom model");
  const after = (await title.boundingBox())!;
  expect(after.x).toBe(before.x);
  expect(after.y).toBe(before.y);
  await page.keyboard.press("Escape");
  await expect(title).toHaveText("Add model");
});
