import { expect, test } from "@playwright/test";
import { open } from "./support";

test.use({ forcedColors: "active" });

test("a highlighted menu row draws its description and key caps in the row's system colours", async ({ page }) => {
  await open(page, "/components/menu/states");
  const menu = page.getByRole("menu").first();
  for (const [name, part] of [
    ["Edit access", "Rates, budgets and model access."],
    ["Rename", "kbd"],
  ] as const) {
    const row = menu.getByRole("menuitem", { name }).first();
    await row.hover();
    await expect(row).toHaveAttribute("data-highlighted", "");
    const inner = part === "kbd" ? row.locator("kbd") : row.getByText(part, { exact: true });
    const colours = await inner.evaluate((el) => {
      const item = el.closest("[data-highlighted]")!;
      return { row: getComputedStyle(item).color, part: getComputedStyle(el).color, fill: getComputedStyle(el).backgroundColor };
    });
    expect(colours.part, `${name}: ${part}`).toBe(colours.row);
    expect(colours.fill, `${name}: ${part}`).toBe("rgba(0, 0, 0, 0)");
  }
});

test("a highlighted select option draws its description in the row's system colour", async ({ page }) => {
  await open(page, "/components/select/examples");
  await page.locator("#ex-rich").getByRole("combobox", { name: "Preset" }).click();
  const option = page.getByRole("option", { name: /^Viewer/ });
  await option.hover();
  await expect(option).toHaveAttribute("data-highlighted", "");
  const colours = await option.getByText("Read usage and logs without content.").evaluate((el) => ({
    row: getComputedStyle(el.closest("[data-highlighted]")!).color,
    part: getComputedStyle(el).color,
  }));
  expect(colours.part).toBe(colours.row);
});
