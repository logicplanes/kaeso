import { expect, test, type Page } from "@playwright/test";
import { hydrated, open } from "./support";

const pane = (page: Page) => page.locator("[data-preview] .pv-stage").first();
test("Switch toggles once with Space", async ({ page }) => {
  await open(page, "/components/switch");
  const toggle = pane(page).locator("[role=switch]").first();
  const before = await toggle.getAttribute("aria-checked");
  await toggle.focus();
  await page.keyboard.press(" ");
  await expect(toggle).not.toHaveAttribute("aria-checked", before ?? "");
});

test("Menu opens with Enter, moves with arrows and returns focus on Escape", async ({ page }) => {
  await open(page, "/components/menu");
  const trigger = pane(page).locator("button[aria-haspopup]").first();
  await trigger.focus();
  await page.keyboard.press("Enter");
  await page.waitForSelector("[role=menu]");
  await page.keyboard.press("ArrowDown");
  const role = await page.evaluate(() => document.activeElement?.getAttribute("role"));
  expect(["menuitem", "menuitemcheckbox", "menuitemradio"]).toContain(role);
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

test("Dialog traps focus and returns it", async ({ page }) => {
  await open(page, "/components/dialog");
  const trigger = pane(page).locator("button").first();
  await trigger.focus();
  await page.keyboard.press("Enter");
  await page.waitForSelector("[role=dialog]");
  const inDialog = () => page.evaluate(() => !!document.activeElement?.closest("[role=dialog]"));
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press("Tab");
    await expect.poll(inDialog).toBe(true);
  }
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

for (const [name, path, id] of [
  ["StepDialog opened from outside returns focus to its opener", "/components/dialog/examples", "steps"],
  ["CommandPalette returns focus to what opened it", "/components/command-palette", "basic"],
] as const) {
  test(name, async ({ page }) => {
    await open(page, path);
    const opener = page.locator(`#ex-${id} .pv-stage button`).first();
    await opener.scrollIntoViewIfNeeded();
    await hydrated(page);
    await opener.focus();
    await page.keyboard.press("Enter");
    await page.waitForSelector("[role=dialog]");
    await expect.poll(() => page.evaluate(() => !!document.activeElement?.closest("[role=dialog]"))).toBe(true);
    await page.keyboard.press("Escape");
    await expect(opener).toBeFocused();
  });
}

test("LayerDialog keeps the focus in the layer it shows, so ⌘Enter submits from each, and Back returns it to the opener", async ({ page }) => {
  await open(page, "/components/layer-dialog");
  const trigger = pane(page).getByRole("button", { name: "Add model" });
  await trigger.focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog").filter({ has: page.locator("[data-dialog-frame]") });
  const back = dialog.getByRole("button", { name: "Back" });
  await expect(dialog).toHaveAccessibleName("Add model");
  await expect(dialog.getByRole("gridcell").first()).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const row = dialog.getByRole("gridcell", { name: /gpt-5-mini/ });
  await expect(row).toBeFocused();

  await page.keyboard.press("Enter");
  await expect(dialog).toHaveAccessibleName("Review gpt-5-mini");
  await expect(dialog.getByRole("button", { name: "Add model" })).toBeFocused();
  await back.focus();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveAccessibleName("Add model");
  await expect(row).toBeFocused();

  await page.keyboard.press("Tab");
  const custom = dialog.getByRole("button", { name: "Add a custom model" });
  await expect(custom).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveAccessibleName("Add a custom model");
  await expect(dialog.getByRole("textbox", { name: "Model id" })).toBeFocused();
  await back.click();
  await expect(custom).toBeFocused();

  await page.keyboard.press("Enter");
  await page.keyboard.type("acme-internal-1");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(dialog).toBeHidden();
  await expect(page.getByText("Added acme-internal-1")).toBeVisible();
  await expect(trigger).toBeFocused();

  await page.keyboard.press("Enter");
  await expect(dialog.getByRole("gridcell").first()).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog.getByRole("button", { name: "Add model" })).toBeFocused();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(dialog).toBeHidden();
  await expect(page.getByText("Added gpt-5", { exact: true })).toBeVisible();
});

test("DataTable header sticks while its rows scroll, and follows them sideways", async ({ page }) => {
  await open(page, "/components/data-table");
  const example = page.locator("#ex-logs .pv-stage").first();
  await example.scrollIntoViewIfNeeded();
  await hydrated(page);
  const result = await example.evaluate((el) => {
    const scroller = el.querySelector<HTMLElement>(".overflow-y-auto")!;
    const table = el.querySelector("[role=table]")!;
    const [head, body] = [...table.querySelectorAll<HTMLElement>(":scope > [role=rowgroup]")];
    scroller.scrollTop = 400;
    body!.scrollLeft = 24;
    return new Promise<{ dy: number; x: number[] }>((resolve) =>
      requestAnimationFrame(() =>
        resolve({ dy: Math.round(head!.getBoundingClientRect().top - scroller.getBoundingClientRect().top), x: [head!.scrollLeft, body!.scrollLeft] }),
      ),
    );
  });
  expect(result.dy).toBe(0);
  expect(result.x[0]).toBe(result.x[1]);
});

test("Segmented moves with arrows", async ({ page }) => {
  await open(page, "/components/segmented");
  const radios = pane(page).locator("[role=radio]");
  const checked = () => radios.evaluateAll((els) => els.findIndex((e) => e.getAttribute("aria-checked") === "true" || e.hasAttribute("data-checked")));
  const start = await checked();
  await radios.nth(start).focus();
  await page.keyboard.press("ArrowRight");
  expect(await checked()).not.toBe(start);
});

test("Tabs move with arrows", async ({ page }) => {
  await open(page, "/components/tabs");
  const tab = pane(page).locator("[role=tab]").first();
  await tab.focus();
  await page.keyboard.press("ArrowRight");
  expect(
    await page.evaluate(
      () => document.activeElement?.getAttribute("role") === "tab" && document.activeElement !== document.querySelector(".pv-stage [role=tab]"),
    ),
  ).toBe(true);
});

test("List rows move with j and k", async ({ page }) => {
  await open(page, "/components/list");
  const rows = pane(page).locator("[data-row]");
  await rows.first().click({ position: { x: 300, y: 10 } });
  await page.keyboard.press("j");
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press("k");
  await expect(rows.first()).toBeFocused();
});

test("List is one tab stop that selects with Space and reaches row actions with the right arrow", async ({ page }) => {
  await open(page, "/components/list");
  const grid = pane(page).locator("[role=grid]").first();
  const rows = grid.locator("[data-row]");
  await rows.first().click({ position: { x: 300, y: 10 } });
  await page.keyboard.press("Space");
  await expect(rows.first()).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Space");
  await expect(rows.first()).toHaveAttribute("aria-selected", "false");
  await page.keyboard.press("ArrowRight");
  await expect(rows.first().getByRole("button", { name: "Copy key id" })).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(rows.first()).toBeFocused();
  await page.keyboard.press("Tab");
  expect(await grid.evaluate((el) => el.contains(document.activeElement))).toBe(false);
  await page.keyboard.press("Shift+Tab");
  await expect(rows.first()).toBeFocused();
});

test("Select opens with Enter and returns focus on Escape", async ({ page }) => {
  await open(page, "/components/select");
  const trigger = pane(page).locator("[role=combobox]").first();
  await trigger.focus();
  await page.keyboard.press("Enter");
  await page.waitForSelector("[role=listbox]");
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

test("⌘K opens the search and Escape closes it", async ({ page }) => {
  await open(page, "/installation");
  await page.keyboard.press("Meta+k");
  await page.waitForSelector("[role=dialog] input");
  expect(await page.evaluate(() => document.activeElement?.tagName)).toBe("INPUT");
  await page.keyboard.press("Escape");
  await expect(page.locator("[role=dialog]")).toHaveCount(0);
});

test("Sidebar collapses with [ and hands focus to the control that brings it back", async ({ page }) => {
  await open(page, "/components/button");
  await page.locator("nav[data-sidebar-root] a").first().focus();
  await page.keyboard.press("[");
  await expect(page.locator("html")).toHaveAttribute("data-sidebar-collapsed", /.*/);
  await expect(page.locator("[data-sidebar-expand]").first()).toBeFocused();
  await page.keyboard.press("[");
  await expect(page.locator("html")).not.toHaveAttribute("data-sidebar-collapsed", /.*/);
});
