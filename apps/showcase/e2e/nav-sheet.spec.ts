import { expect, test, type Page, type Route } from "@playwright/test";
import { hydrated, open, settle } from "./support";

const PEEK_DELAY = 750;
const NAV_SHEET = /\/nav-sheet\.[\w-]+\.js(\?retry=\d+)?$/;

async function isSheetEntry(route: Route) {
  const response = await route.fetch();
  return { response, entry: (await response.text()).startsWith("export{") };
}

async function holdSheet(page: Page) {
  let release!: () => void;
  const arrived = new Promise<void>((resolve) => (release = resolve));
  const asked: string[] = [];
  await page.route(NAV_SHEET, async (route) => {
    const { response, entry } = await isSheetEntry(route);
    if (entry) {
      asked.push(route.request().url());
      await arrived;
    }
    await route.fulfill({ response });
  });
  return { release, asked };
}

const phoneBar = (page: Page) => page.locator("[data-phone-bar]").first();
const sheet = (page: Page) => page.getByRole("dialog", { name: "Navigation" });

test.describe("phone width", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the navigation toggle opens the sheet once the network is back after its code failed to load", async ({ page }) => {
    let offline = true;
    const asked: string[] = [];
    await page.route(NAV_SHEET, async (route) => {
      const { response, entry } = await isSheetEntry(route);
      if (!entry) return route.fulfill({ response });
      asked.push(new URL(route.request().url()).search);
      if (offline) return route.abort("internetdisconnected");
      return route.fulfill({ response });
    });
    await open(page, "/components/app-frame");
    expect(asked).toEqual([""]);
    const toggle = phoneBar(page).getByRole("button", { name: "Open navigation" });
    await toggle.scrollIntoViewIfNeeded();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");

    await toggle.tap();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(sheet(page)).toHaveCount(0);
    expect(asked).toEqual(["", "?retry=1"]);

    offline = false;
    await toggle.tap();
    await expect(sheet(page).getByRole("button", { name: "Overview" })).toBeVisible();
    expect(asked).toEqual(["", "?retry=1", "?retry=2"]);
  });

  test("a tap taken back with Escape before the sheet's code arrives never opens the sheet late", async ({ page }) => {
    const held = await holdSheet(page);
    await page.goto("/components/app-frame");
    const toggle = phoneBar(page).getByRole("button", { name: "Open navigation" });
    await toggle.scrollIntoViewIfNeeded();
    await hydrated(page);
    await expect.poll(() => held.asked.length, { message: "the sheet's code is on its way" }).toBeGreaterThan(0);

    await toggle.tap();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Escape");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await toggle.tap();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await toggle.tap();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");

    held.release();
    await settle(page);
    await expect(sheet(page)).toHaveCount(0);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await toggle.tap();
    await expect(sheet(page).getByRole("button", { name: "Overview" })).toBeVisible();
  });
});

test("the collapsed sidebar never peeks after the pointer left before the sheet's code arrived", async ({ page }) => {
  const held = await holdSheet(page);
  await page.goto("/components/app-frame");
  const frame = page.locator("main").getByRole("navigation", { name: "Main" }).first();
  await frame.scrollIntoViewIfNeeded();
  await hydrated(page);
  await frame.getByRole("button", { name: "Collapse sidebar" }).click();
  const expand = page.getByRole("button", { name: "Expand sidebar" }).first();
  await expect(expand).toBeVisible();

  await page.clock.install();
  await expand.hover();
  await page.clock.runFor(PEEK_DELAY);
  await page.mouse.move(720, 720);
  await page.clock.resume();
  held.release();
  await settle(page);
  await expect(sheet(page)).toHaveCount(0);

  await expand.hover();
  await expect(sheet(page).getByRole("button", { name: "Overview" })).toBeVisible();
});
