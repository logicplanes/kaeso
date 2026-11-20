import { devices, type Browser, type Page } from "@playwright/test";
import { expect, test, signInToProject } from "./support";

test.describe.configure({ mode: "serial" });

const LONG_KEY = "checkout-web-production-eu-central-frankfurt-blue-green-deployment";
const LONG_ALIAS = "checkout-assistant-primary-reply-with-fallbacks-for-peak-traffic";

interface Place {
  name: string;
  path: string;
  dialog?: boolean;
}

// Runs in the page: what makes it scroll sideways. Wide content may scroll inside its own container, never the page.
function sideways(): string[] {
  const width = window.innerWidth;
  const found: string[] = [];
  const describe = (el: Element) => `<${el.tagName.toLowerCase()}> "${(el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 40)}"`;
  if (document.documentElement.scrollWidth > width) found.push(`the page is ${document.documentElement.scrollWidth}px wide`);
  for (const body of document.querySelectorAll("main > div.overflow-y-auto"))
    if (body.scrollWidth > body.clientWidth + 1) found.push(`the page body scrolls sideways: ${body.scrollWidth}px in ${body.clientWidth}px`);
  for (const el of document.querySelectorAll("body *")) {
    const box = el.getBoundingClientRect();
    if (!box.width || !box.height || (box.right <= width + 1 && box.left >= -1) || el.closest("[aria-hidden=true]")) continue;
    let held = false;
    for (let up = el.parentElement; up && !held; up = up.parentElement) {
      const around = up.getBoundingClientRect();
      held = /(hidden|auto|scroll|clip)/.test(getComputedStyle(up).overflowX) && around.right <= width + 1 && around.left >= -1;
    }
    if (!held) found.push(`${describe(el)} reaches ${Math.round(box.left)}–${Math.round(box.right)}px`);
  }
  return [...new Set(found)].slice(0, 8);
}

let places: Place[] = [];

async function visitAll(browser: Browser, page: Page, device: keyof typeof devices, width: number, height: number) {
  const { defaultBrowserType: _, ...emulated } = devices[device];
  const origin = new URL(page.url()).origin;
  const context = await browser.newContext({ ...emulated, viewport: { width, height }, storageState: await page.context().storageState() });
  const phone = await context.newPage();
  const problems: string[] = [];
  for (const place of places) {
    await phone.goto(`${origin}${place.path}`);
    await expect(phone.locator("main")).toBeVisible();
    if (place.dialog) await expect(phone.getByRole("dialog").first()).toBeVisible();
    await expect(phone.locator("[aria-busy=true]")).toHaveCount(0);
    await phone.evaluate(async () => void (await document.fonts.ready));
    for (const problem of await phone.evaluate(sideways)) problems.push(`${place.name}: ${problem}`);
  }
  await context.close();
  expect(problems, `pages that scroll sideways at ${width}px`).toEqual([]);
}

test("every project page, sheet and dialog fits a phone and a tablet without scrolling sideways", async ({ page, browser }) => {
  test.setTimeout(240_000);
  await signInToProject(page);
  const origin = new URL(page.url()).origin;
  const [, , org, , project] = new URL(page.url()).pathname.split("/");
  const base = `/orgs/${org}/projects/${project}`;
  const api = `/platform/v1${base}`;
  const headers = { origin };
  const key = (await (await page.request.post(`${api}/keys`, { headers, data: { name: LONG_KEY, access: "inference" } })).json()) as { id: string };
  const models = (await (await page.request.get(`${api}/models`)).json()) as { data: { id: string }[] };
  const model = models.data[0]?.id;
  if (model) expect((await page.request.post(`${api}/aliases`, { headers, data: { name: LONG_ALIAS, route: { type: "model", model } } })).ok()).toBe(true);
  const me = (await (await page.request.get("/platform/v1/me")).json()) as { id: string };
  const logs = (await (await page.request.get(`${api}/logs?limit=1`)).json()) as { data: { id: string }[] };

  places = [
    { name: "Overview", path: base },
    { name: "Playground", path: `${base}/playground` },
    { name: "API keys", path: `${base}/keys` },
    { name: "Key sheet", path: `${base}/keys?key=${key.id}`, dialog: true },
    { name: "Create key", path: `${base}/keys?create=true`, dialog: true },
    { name: "Request logs", path: `${base}/logs` },
    ...(logs.data[0] ? [{ name: "Request inspector", path: `${base}/logs?log=${logs.data[0].id}`, dialog: true }] : []),
    { name: "Tool calls", path: `${base}/logs?kind=tools` },
    { name: "Models", path: `${base}/models` },
    ...(model ? [{ name: "Model sheet", path: `${base}/models?model=${model}`, dialog: true }] : []),
    { name: "Add models", path: `${base}/models?add=true`, dialog: true },
    { name: "Aliases", path: `${base}/models?tab=aliases` },
    { name: "Retiring models", path: `${base}/models?tab=retiring` },
    { name: "Tools", path: `${base}/tools` },
    { name: "MCP servers", path: `${base}/tools?tab=servers` },
    { name: "Connect a client", path: `${base}/tools?tab=connect` },
    { name: "Limits", path: `${base}/limits` },
    { name: "Add limit", path: `${base}/limits?add=true`, dialog: true },
    { name: "Members", path: `${base}/members` },
    { name: "Member sheet", path: `${base}/members?member=${me.id}`, dialog: true },
    { name: "Add member", path: `${base}/members?add=true`, dialog: true },
    { name: "Service accounts", path: `${base}/service-accounts` },
    { name: "Create service account", path: `${base}/service-accounts?create=true`, dialog: true },
    { name: "Audit log", path: `${base}/audit-log` },
    { name: "Settings", path: `${base}/settings` },
  ];
  await visitAll(browser, page, "iPhone 15", 390, 844);
  await visitAll(browser, page, "iPad (gen 11)", 820, 1180);
});
