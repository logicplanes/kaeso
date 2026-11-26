import type { Browser, Page } from "@playwright/test";
import { expect, test, signInToProject } from "./support";

test.describe.configure({ mode: "serial" });

type Measured = { scrollWidth: number; clientWidth: number };

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const projectOf = (page: Page) => /\/projects\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";

// A phone the size of an iPhone 15 in Chromium: touch, a mobile viewport and 16px inputs.
async function onPhone(browser: Browser, page: Page): Promise<Page> {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
    storageState: await page.context().storageState(),
  });
  return context.newPage();
}

// What reaches past the screen's right edge: the page itself, or an element no scrolling parent holds.
const OVERFLOW = `(() => {
  const width = innerWidth;
  const found = [];
  if (document.documentElement.scrollWidth > width) found.push("the page is " + document.documentElement.scrollWidth + "px wide");
  for (const el of document.querySelectorAll("body *")) {
    const box = el.getBoundingClientRect();
    if (!box.width || !box.height || box.right <= width + 1 || getComputedStyle(el).position === "fixed") continue;
    let held = false;
    for (let up = el.parentElement; up && !held; up = up.parentElement) {
      held = /(auto|scroll|hidden|clip)/.test(getComputedStyle(up).overflowX) && up.getBoundingClientRect().right <= width + 1;
    }
    if (!held) found.push(el.tagName.toLowerCase() + ' "' + (el.textContent ?? "").trim().slice(0, 40) + '" ends at ' + Math.round(box.right) + "px");
  }
  return found;
})()`;

// Scrolls every scroller to its foot, so what renders late is measured too.
const TO_THE_FOOT = `(() => {
  for (const el of document.querySelectorAll("*")) {
    const style = getComputedStyle(el).overflowY;
    if ((style === "auto" || style === "scroll") && el.scrollHeight > el.clientHeight) el.scrollTop = el.scrollHeight;
  }
})()`;

const overflow = (page: Page) => page.evaluate(OVERFLOW) as Promise<string[]>;

async function fits(page: Page, path: string): Promise<void> {
  await page.goto(path);
  await expect(page.getByRole("heading", { level: 1, name: "Analytics" })).toBeVisible({ timeout: 20_000 });
  await expect(page.locator("[aria-busy=true]")).toHaveCount(0, { timeout: 20_000 });
  await page.evaluate(TO_THE_FOOT);
  expect(await overflow(page), path).toEqual([]);
}

test("analytics fits a phone on every view and grouping, and its chart answers a tap", async ({ browser, page }) => {
  await signInToProject(page);
  const origin = new URL(page.url()).origin;
  const org = orgOf(page);
  const project = projectOf(page);
  const headers = { origin };
  // Each CI part runs on a stack of its own, so this spec brings its own provider key and model.
  const credentials = (await (await page.request.get(`/platform/v1/orgs/${org}/credentials`)).json()) as { data: { provider: string }[] };
  if (!credentials.data.some((c) => c.provider === "openai")) {
    const added = await page.request.post(`/platform/v1/orgs/${org}/credentials`, {
      headers,
      data: { provider: "openai", name: "Mock", secret: "sk-proj-mock-0000000000000000openai" },
    });
    expect(added.ok()).toBe(true);
  }
  const models = (await (await page.request.get(`/platform/v1/orgs/${org}/projects/${project}/models`)).json()) as { data: { model_id: string }[] };
  if (!models.data.some((m) => m.model_id === "gpt-5.4-mini")) {
    const added = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/models`, {
      headers,
      data: { provider: "openai", model_id: "gpt-5.4-mini" },
    });
    expect(added.ok()).toBe(true);
  }
  const created = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/keys`, {
    headers,
    data: { name: "phone-analytics-with-a-long-key-name", access: "inference" },
  });
  expect(created.ok()).toBe(true);
  const { id: keyId, secret } = (await created.json()) as { id: string; secret: string };
  const call = (content: string) =>
    page.request.post("/v1/chat/completions", {
      headers: { authorization: `Bearer ${secret}` },
      data: { model: "gpt-5.4-mini", messages: [{ role: "user", content }] },
    });
  for (const content of ["One", "Two"]) expect((await call(content)).status()).toBe(200);
  expect((await call("Three [mock:error=server-error]")).status()).toBeGreaterThanOrEqual(400);
  // The long key name has to be in the usage before the pages are measured.
  await expect(async () => {
    const usage = (await (await page.request.get(`/platform/v1/orgs/${org}/projects/${project}/usage?group_by=key&key=${keyId}`)).json()) as {
      data: { requests: number }[];
    };
    expect(usage.data.reduce((a, r) => a + r.requests, 0)).toBeGreaterThanOrEqual(3);
  }).toPass({ timeout: 30_000 });

  const phone = await onPhone(browser, page);
  const projectPath = `${origin}/orgs/${org}/projects/${project}/analytics`;
  const orgPath = `${origin}/orgs/${org}/analytics`;
  for (const path of [
    projectPath,
    ...["model", "key", "member", "route", "client"].map((by) => `${projectPath}/breakdown?by=${by}&period=7d`),
    ...["reliability", "efficiency", "models", "keys", "members", "breakdown"].map((view) => `${projectPath}/${view}`),
    `${projectPath}?period=today`,
    `${projectPath}?period=12m`,
    orgPath,
    ...["model", "member", "billed_to"].map((by) => `${orgPath}/breakdown?by=${by}`),
    ...["reliability", "efficiency", "projects", "models", "members", "breakdown"].map((view) => `${orgPath}/${view}`),
  ])
    await fits(phone, path);

  await phone.goto(`${projectPath}/breakdown`);
  const groupBy = phone.getByRole("combobox", { name: "Group by" });
  await expect(groupBy).toHaveText(/By day/);
  await groupBy.tap();
  await phone.getByRole("option", { name: "By model" }).tap();
  await expect.poll(() => new URL(phone.url()).searchParams.get("by")).toBe("model");

  await phone.goto(projectPath);
  await phone.getByRole("button", { description: "Period" }).tap();
  const picker = phone.getByRole("dialog", { name: "Choose a date range" });
  await expect(picker).toBeVisible();
  for (const name of ["Start date", "End date"]) {
    const field = picker.getByRole("textbox", { name });
    expect(await field.evaluate((el) => (el as unknown as Measured).scrollWidth <= (el as unknown as Measured).clientWidth)).toBe(true);
  }
  expect(await overflow(phone)).toEqual([]);
  await phone.keyboard.press("Escape");

  const chart = phone.locator("#overview-chart");
  await chart.scrollIntoViewIfNeeded();
  const plot = await chart.locator("svg rect[fill=transparent]").last().boundingBox();
  if (!plot) throw new Error("The chart has no plot to tap");
  const tooltip = chart.locator("div[role=presentation]");
  await phone.touchscreen.tap(plot.x + plot.width - 4, plot.y + plot.height / 2);
  await expect(tooltip).toBeVisible();
  await expect(tooltip).toContainText(/\d/);
  await phone.getByRole("heading", { level: 1, name: "Analytics" }).tap();
  await expect(tooltip).toHaveCount(0);
  await phone.context().close();
});
