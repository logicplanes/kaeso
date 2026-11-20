import { readFileSync } from "node:fs";
import { devices, type Cookie, type Page } from "@playwright/test";
import { expect, test, firstProjectOf, NEW_PASSWORD, OWNER } from "./support";

// The routes come from the generated route tree, so a new page is checked without being listed here.
const { defaultBrowserType: _browser, ...phone } = devices["iPhone 15"];
test.use({ ...phone, viewport: { width: 390, height: 844 } });

const tree = readFileSync(new URL("../src/routeTree.gen.ts", import.meta.url), "utf8");
const byFullPath = tree.slice(tree.indexOf("export interface FileRoutesByFullPath {"));
const ROUTES = [...byFullPath.slice(0, byFullPath.indexOf("\n}")).matchAll(/^ {2}'(\/[^']*)': typeof /gm)].map((m) => m[1] ?? "");
const PUBLIC = /^\/(authorize|invite|setup|sign-in|sign-up|signed-in-again|unsubscribe|auth\/|me\/email\/|share\/)/;
const VIEWS = { org: ["projects", "models", "members"], project: ["models", "keys", "members"] };
const TABS: Record<string, string[]> = {
  "/orgs/$org/projects/$project/models": ["?tab=aliases", "?tab=retiring"],
  "/settings/orgs/$org/members": ["?tab=invites", "?tab=roles"],
  "/settings/orgs/$org/logging": ["?tab=storage", "?tab=deletions"],
};

interface Ids {
  org: string;
  project: string;
  share: string;
}
let ids: Ids = { org: "", project: "", share: "" };

// Signed in once for the file: the gateway limits sign-in attempts, and the shared helper waits for the sidebar,
// which a phone keeps in its navigation sheet.
let cookies: Cookie[] = [];

test.beforeAll(async ({ browser }) => {
  const context = await browser.newContext();
  try {
    const page = await context.newPage();
    await page.goto("/sign-in");
    const headers = { origin: new URL(page.url()).origin };
    expect((await page.request.post("/platform/v1/auth/password", { headers, data: { email: OWNER.email, password: NEW_PASSWORD } })).ok()).toBe(true);
    const memberships = (await (await page.request.get("/platform/v1/me/memberships")).json()) as { data: { org_id: string }[] };
    const org = memberships.data[0]?.org_id ?? "";
    const project = await firstProjectOf(page, org);
    const api = `/platform/v1/orgs/${org}`;
    const credentials = (await (await page.request.get(`${api}/credentials`)).json()) as { data: { provider: string }[] };
    // A provider key's row is long enough on a phone to end in an ellipsis, where hidden labels once widened the page.
    if (!credentials.data.some((c) => c.provider === "openai"))
      expect(
        (
          await page.request.post(`${api}/credentials`, {
            headers,
            data: { provider: "openai", name: "Production", secret: "sk-proj-mock-0000000000000000openai" },
          })
        ).ok(),
      ).toBe(true);
    expect((await page.request.put(`${api}/security/shared-links`, { headers, data: { allow: "anyone", max_days: 90 } })).ok()).toBe(true);
    const link = await page.request.post(`${api}/shared-links`, { headers, data: { kind: "analytics", audience: "anyone", expires_in_days: 30 } });
    expect(link.ok()).toBe(true);
    ids = { org, project, share: new URL(((await link.json()) as { url: string }).url).pathname.split("/").pop() ?? "" };
    cookies = await context.cookies();
  } finally {
    await context.close();
  }
});

function urls(route: string, { org, project, share }: Ids): string[] {
  const filled = (view: string) =>
    route
      .replace("$org", org)
      .replace("$project", project)
      .replace("$view", view)
      .replace("$alias", "no-such-alias")
      .replace("$agent", "agt_00000000000000000000000000")
      .replace("$token", share)
      .replace(/\/\$$/, "/no-such-page");
  const views = route.includes("$view") ? (route.includes("$project") ? VIEWS.project : VIEWS.org) : [""];
  return views.flatMap((view) => [filled(view), ...(TABS[route] ?? []).map((tab) => filled(view) + tab)]);
}

// In the page: what scrolls sideways, and the outermost elements that reach past the scroller's edge.
const SIDEWAYS = `(() => {
  const describe = (el) => {
    const text = (el.textContent ?? "").trim().replace(/\\s+/g, " ").slice(0, 40);
    return "<" + el.tagName.toLowerCase() + ' class="' + (el.getAttribute("class") ?? "").slice(0, 60) + '">' + (text ? ' "' + text + '"' : "");
  };
  const root = document.scrollingElement ?? document.documentElement;
  const problems = [];
  for (const scroller of [root, ...document.querySelectorAll("[data-frame-body], [data-dialog-body]")]) {
    const by = scroller.scrollWidth - scroller.clientWidth;
    if (by <= 0) continue;
    const edge = scroller === root ? window.innerWidth : scroller.getBoundingClientRect().left + scroller.clientWidth;
    const clipped = (el) => {
      // Ancestors below its containing block don't clip a positioned element: that is how hidden text once widened pages.
      let a = el.parentElement;
      if (["absolute", "fixed"].includes(getComputedStyle(el).position)) while (a && a !== el.offsetParent) a = a.parentElement;
      for (; a && a !== scroller; a = a.parentElement)
        if (getComputedStyle(a).overflowX !== "visible" && a.getBoundingClientRect().right <= edge + 0.5) return true;
      return false;
    };
    const past = [];
    for (const el of scroller.querySelectorAll("*"))
      if (el.getBoundingClientRect().right > edge + 0.5 && !past.some((p) => p.contains(el)) && !clipped(el)) past.push(el);
    problems.push((scroller === root ? "the page" : describe(scroller)) + " scrolls " + by + "px sideways; past its edge: " + past.slice(0, 3).map(describe).join(", "));
  }
  return problems;
})()`;

// Scrolls the page to its end, a screen at a time, so lazy content gets drawn.
const TO_THE_END = `(async () => {
  for (const el of [document.scrollingElement, ...document.querySelectorAll("[data-frame-body]")]) {
    if (!el) continue;
    for (let y = 0; y < el.scrollHeight; y += el.clientHeight) {
      el.scrollTop = y;
      await new Promise((r) => requestAnimationFrame(r));
    }
  }
})()`;

// The page shows its data once the console's requests to the gateway have been quiet for a moment.
async function settled(page: Page): Promise<void> {
  const open = new Set<unknown>();
  let last = Date.now();
  const track = (r: { url(): string }) => r.url().includes("/platform/") && open.add(r) && (last = Date.now());
  const done = (r: unknown) => open.delete(r) && (last = Date.now());
  page.on("request", track);
  page.on("requestfinished", done);
  page.on("requestfailed", done);
  try {
    await expect.poll(() => open.size === 0 && Date.now() - last >= 500, { intervals: [100] }).toBe(true);
  } finally {
    page.off("request", track);
    page.off("requestfinished", done);
    page.off("requestfailed", done);
  }
}

async function check(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await settled(page);
  await page.evaluate(TO_THE_END);
  await settled(page);
  expect(await page.evaluate<string[]>(SIDEWAYS), url).toEqual([]);
}

test("the route list holds every page of the console", () => {
  expect(ROUTES).toContain("/orgs/$org/projects/$project/logs");
  expect(ROUTES.length).toBeGreaterThan(60);
});

for (const route of ROUTES) {
  test(`${route} fits a phone without scrolling sideways`, async ({ page, browser }) => {
    await page.context().addCookies(cookies);
    for (const url of urls(route, ids)) await check(page, url);
    if (!PUBLIC.test(route)) return;
    const signedOut = await browser.newContext({ ...phone, viewport: { width: 390, height: 844 } });
    try {
      const out = await signedOut.newPage();
      for (const url of urls(route, ids)) await check(out, url);
    } finally {
      await signedOut.close();
    }
  });
}
