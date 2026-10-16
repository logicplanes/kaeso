import type { Page } from "@playwright/test";
import { expect, test, signInAsOwner } from "./support";

test.describe.configure({ mode: "serial" });

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const breakdownRows = (page: Page) =>
  page
    .getByRole("table", { name: "Usage breakdown" })
    .getByRole("row")
    .filter({ hasNot: page.getByRole("columnheader") });

type Scroller = { scrollTop: number; scrollHeight: number };

async function projectWithUsage(page: Page, org: string, name: string, models: string[], requests: number) {
  const headers = { origin: new URL(page.url()).origin };
  const api = `/platform/v1/orgs/${org}`;
  const project = ((await (await page.request.post(`${api}/projects`, { headers, data: { name } })).json()) as { id: string }).id;
  const base = `${api}/projects/${project}`;
  const ids: string[] = [];
  for (const q of models) {
    const hits = (
      (await (await page.request.get(`${base}/models/search?provider=openai&q=${encodeURIComponent(q)}`)).json()) as { data: { model_id: string }[] }
    ).data;
    const hit = hits.find((h) => h.model_id === q);
    if (hit) ids.push(hit.model_id);
  }
  expect(ids).toEqual(models);
  expect((await page.request.post(`${base}/models/batch`, { headers, data: { provider: "openai", model_ids: ids } })).ok()).toBe(true);
  const key = (await (await page.request.post(`${base}/keys`, { headers, data: { name: `${name} key`, access: "inference" } })).json()) as { secret: string };
  for (let i = 0; i < requests; i++) {
    const res = await page.request.post("/v1/chat/completions", {
      headers: { authorization: `Bearer ${key.secret}` },
      data: { model: models[i % models.length], messages: [{ role: "user", content: `Switch ${i}` }] },
    });
    expect(res.status()).toBe(200);
  }
  await expect(async () => {
    const usage = (await (await page.request.get(`${base}/usage?group_by=model`)).json()) as { data: { key: string }[] };
    expect(usage.data.length).toBe(models.length);
  }).toPass({ timeout: 30_000 });
  return project;
}

test("switching projects client-side shows the new project's data, at its top, without an error; Back returns to the scroll", async ({ page }) => {
  await signInAsOwner(page);
  const org = orgOf(page);
  const headers = { origin: new URL(page.url()).origin };
  const credentials = (await (await page.request.get(`/platform/v1/orgs/${org}/credentials`)).json()) as { data: { provider: string }[] };
  if (!credentials.data.some((c) => c.provider === "openai")) {
    const res = await page.request.post(`/platform/v1/orgs/${org}/credentials`, {
      headers,
      data: { provider: "openai", name: "Mock", secret: "sk-proj-mock-0000000000000000openai" },
    });
    expect(res.ok()).toBe(true);
  }
  const wide = await projectWithUsage(page, org, "Switch wide", ["gpt-5.4-mini", "gpt-5.4-nano"], 4);
  const narrow = await projectWithUsage(page, org, "Switch narrow", ["gpt-5.4-nano"], 1);

  await page.goto(`/orgs/${org}/projects/${wide}?by=model`);
  await expect(breakdownRows(page)).toHaveCount(2);

  const body = page.locator("main > div.overflow-y-auto");
  await body.evaluate((el) => void ((el as unknown as Scroller).scrollTop = (el as unknown as Scroller).scrollHeight));
  await expect.poll(() => body.evaluate((el) => (el as unknown as Scroller).scrollTop)).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Switch wide: switch project" }).click();
  await page.getByRole("menuitemradio", { name: "Switch narrow" }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${narrow}\\?by=model$`));
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Switch narrow: switch project" })).toBeVisible();
  await expect(breakdownRows(page)).toHaveCount(1);
  await expect(breakdownRows(page).first()).toContainText("gpt-5.4-nano");
  await expect(page.getByRole("figure", { name: "Requests and errors per day" })).toBeVisible();
  expect(await page.locator("main > div.overflow-y-auto").evaluate((el) => (el as unknown as Scroller).scrollTop)).toBe(0);

  await page.goBack();
  await expect(page).toHaveURL(new RegExp(`/projects/${wide}\\?by=model$`));
  await expect(breakdownRows(page)).toHaveCount(2);
  await expect.poll(() => body.evaluate((el) => (el as unknown as Scroller).scrollTop)).toBeGreaterThan(0);
  await page.goForward();
  await expect(page).toHaveURL(new RegExp(`/projects/${narrow}\\?by=model$`));
  await expect(breakdownRows(page)).toHaveCount(1);
  expect(await body.evaluate((el) => (el as unknown as Scroller).scrollTop)).toBe(0);

  await page.getByRole("button", { name: "Switch narrow: switch project" }).click();
  await page.getByRole("menuitemradio", { name: "Switch wide" }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${wide}\\?by=model$`));
  await expect(breakdownRows(page)).toHaveCount(2);

  await page.goto(`/orgs/${org}/projects`);
  await page.getByRole("main").getByRole("row").filter({ hasText: "Switch narrow" }).first().click();
  await expect(page).toHaveURL(new RegExp(`/projects/${narrow}`));
  await expect(page.getByRole("button", { name: "Switch narrow: switch project" })).toBeVisible();
});

async function menuFrames(page: Page, ms: number): Promise<string[]> {
  return page.evaluate(
    (span) =>
      new Promise<string[]>((done) => {
        type El = {
          offsetWidth: number;
          offsetHeight: number;
          closest(s: string): El | null;
          getBoundingClientRect(): { x: number; y: number };
          querySelectorAll(s: string): { length: number };
        };
        const win = globalThis as unknown as { document: { querySelector(s: string): El | null }; requestAnimationFrame(f: () => void): number };
        const doc = win.document;
        const frames: string[] = [];
        const start = performance.now();
        const tick = () => {
          const menu = doc.querySelector('[role="menu"]');
          const at = menu?.closest('[role="presentation"][data-side]')?.getBoundingClientRect();
          if (menu && at)
            frames.push(
              `${Math.round(at.x)},${Math.round(at.y)} ${menu.offsetWidth}x${menu.offsetHeight} ${menu.querySelectorAll('[role^="menuitem"]').length} items`,
            );
          if (performance.now() - start < span) win.requestAnimationFrame(tick);
          else done(frames);
        };
        win.requestAnimationFrame(tick);
      }),
    ms,
  );
}

async function openFrames(page: Page, trigger: ReturnType<Page["getByRole"]>, wait = 400): Promise<string[]> {
  const frames = menuFrames(page, wait + 200);
  await trigger.click();
  return frames;
}

test("the sidebar's switchers open whole the first time: the menu's box in its first frame is its box after", async ({ page }) => {
  await signInAsOwner(page);
  const org = orgOf(page);
  const projects = ((await (await page.request.get(`/platform/v1/orgs/${org}/projects?limit=100`)).json()) as { data: { id: string; name: string }[] }).data;
  const project = projects[0]?.id ?? "";
  const projectSwitcher = page.getByRole("button", { name: /: switch project$/ });
  const orgSwitcher = page.getByRole("button", { name: /: switch or open settings$/ });

  for (const trigger of [projectSwitcher, orgSwitcher]) {
    await page.goto(`/orgs/${org}/projects/${project}`);
    const frames = await openFrames(page, trigger);
    expect(frames.length).toBeGreaterThan(3);
    expect(new Set(frames), frames.join("\n")).toEqual(new Set([frames[0]]));
    if (trigger === projectSwitcher) expect(frames[0]).toContain(`${projects.length + 1} items`);
    await page.keyboard.press("Escape");
  }

  await page.route(/\/sidebar-menus(\.tsx|-[\w-]{8}\.js)(\?.*)?$/, async (route) => {
    await new Promise((r) => setTimeout(r, 1500));
    await route.continue();
  });
  await page.goto(`/orgs/${org}/projects/${project}`);
  const frames = await openFrames(page, projectSwitcher, 2500);
  expect(frames.length).toBeGreaterThan(3);
  expect(new Set(frames), frames.join("\n")).toEqual(new Set([frames[0]]));
  expect(frames[0]).toContain(`${projects.length + 1} items`);
});
