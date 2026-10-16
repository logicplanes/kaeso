import type { Page } from "@playwright/test";
import { expect, test, signInToProject } from "./support";

test.describe.configure({ mode: "serial" });

type Scroller = { scrollHeight: number; clientHeight: number };

const trigger = (page: Page) => page.getByRole("button", { name: "Period" });
const popover = (page: Page) => page.getByRole("dialog", { name: "Choose a date range" });
const search = (page: Page) => new URL(page.url()).searchParams;
const thisMonth = () => new Date(`${new Date().toISOString().slice(0, 10)}T12:00:00`).toLocaleDateString("en-US", { month: "long", year: "numeric" });

async function openPicker(page: Page) {
  await trigger(page).hover();
  await trigger(page).click();
  await expect(popover(page)).toBeVisible();
}

async function pick(page: Page, preset: string) {
  await openPicker(page);
  await popover(page).getByRole("option", { name: preset, exact: true }).click();
  await expect(popover(page)).toBeHidden();
}

test("the overview's presets: a short list, and the calendars on the period's months", async ({ page }) => {
  await signInToProject(page);
  await openPicker(page);
  const presets = popover(page).getByRole("listbox", { name: "Presets" });
  await expect(presets.getByRole("option")).toHaveText([
    "Today",
    "Last 7 days",
    "Last 30 days",
    "Last 90 days",
    "Last 6 months",
    "Last 12 months",
    "All time",
    "Custom",
  ]);
  expect(await presets.evaluate((el) => (el as unknown as Scroller).scrollHeight <= (el as unknown as Scroller).clientHeight + 1)).toBe(true);
  await expect(popover(page).getByRole("grid").last()).toHaveAccessibleName(thisMonth());
  await page.keyboard.press("Escape");
});

test("a preset stays itself in the address, through a reload", async ({ page }) => {
  await signInToProject(page);
  await pick(page, "Last 6 months");
  await expect.poll(() => search(page).get("period")).toBe("6m");
  expect(search(page).get("from")).toBeNull();
  await expect(page.getByText("Requests and errors per week (UTC)")).toBeVisible();
  await page.reload();
  expect(search(page).get("period")).toBe("6m");
  await expect(trigger(page)).toHaveText("Last 6 months");
  await expect(page.getByText("Requests and errors per week (UTC)")).toBeVisible();
  await expect(page.getByText("By kind, per week (UTC)")).toBeVisible();
  await openPicker(page);
  await expect(popover(page).getByRole("grid").last()).toHaveAccessibleName(thisMonth());
  await page.keyboard.press("Escape");

  await pick(page, "Last 12 months");
  await expect.poll(() => search(page).get("period")).toBe("12m");
  await expect(page.getByText("Requests and errors per week (UTC)")).toBeVisible();

  await pick(page, "All time");
  await expect.poll(() => search(page).get("period")).toBe("all");
  await expect(page.getByLabel("Usage in the period")).not.toContainText("vs. prior");
  await page.reload();
  expect(search(page).get("period")).toBe("all");
  await expect(trigger(page)).toHaveText("All time");

  await pick(page, "Last 7 days");
  await expect.poll(() => search(page).get("period")).toBe("7d");
  await expect(page.getByText("Requests and errors per day (UTC)")).toBeVisible();
});

test("custom days go into the address as days, and an unknown preset is the default period", async ({ page }) => {
  await signInToProject(page);
  const today = new Date().toISOString().slice(0, 10);
  const start = new Date(Date.now() - 20 * 86_400_000).toISOString().slice(0, 10);
  const before = page.url();
  await openPicker(page);
  await popover(page).getByRole("textbox", { name: "Start date" }).fill(start);
  await page.keyboard.press("Escape");
  await expect(popover(page)).toBeHidden();
  expect(page.url()).toBe(before);
  await openPicker(page);
  const from = popover(page).getByRole("textbox", { name: "Start date" });
  await from.fill(start);
  await from.press("Enter");
  await expect(popover(page)).toBeVisible();
  const to = popover(page).getByRole("textbox", { name: "End date" });
  await to.fill(today);
  await to.press("Enter");
  await popover(page).getByRole("button", { name: "Apply" }).click();
  await expect.poll(() => [search(page).get("from"), search(page).get("to"), search(page).get("period")]).toEqual([start, today, null]);
  await page.reload();
  expect([search(page).get("from"), search(page).get("to")]).toEqual([start, today]);

  await page.goto(`${new URL(page.url()).pathname}?period=5y`);
  await expect(trigger(page)).toBeVisible();
  await expect(page.getByText("Requests and errors per day (UTC)")).toBeVisible();
});

test("the organization's overview takes the same periods", async ({ page }) => {
  await signInToProject(page);
  const org = /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/orgs/${org}?period=12m`);
  await expect(page.getByText("Requests and errors per week (UTC)")).toBeVisible();
  await pick(page, "Last 90 days");
  await expect.poll(() => search(page).get("period")).toBe("90d");
  await expect(page.getByText("Requests and errors per day (UTC)")).toBeVisible();
  await page.goto(`/settings/orgs/${org}/usage?period=all`);
  await expect.poll(() => new URL(page.url()).pathname).toBe(`/orgs/${org}`);
  expect(search(page).get("period")).toBe("all");
});
