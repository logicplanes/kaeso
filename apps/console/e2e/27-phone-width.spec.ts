import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import { devices, type Cookie, type Page } from "@playwright/test";
import { expect, test, firstProjectOf, NEW_PASSWORD, OWNER } from "./support";

// Without its WebKit default: a device's browser can't change inside a file, and CI runs Chromium.
const { defaultBrowserType: _browser, ...phone } = devices["iPhone 15"];
// Without motion: a sheet or row still fading in would read as low contrast.
const PHONE = { ...phone, viewport: { width: 390, height: 844 }, reducedMotion: "reduce" as const };
test.use(PHONE);

// Read from the generated route tree and the views' list, so a new page or view is checked without being listed here.
const tree = readFileSync(new URL("../src/routeTree.gen.ts", import.meta.url), "utf8");
const byFullPath = tree.slice(tree.indexOf("export interface FileRoutesByFullPath {"));
const ROUTES = [
  ...new Set([...byFullPath.slice(0, byFullPath.indexOf("\n}")).matchAll(/^ {2}'(\/[^']*)': typeof /gm)].map((m) => (m[1] ?? "").replace(/(.)\/$/, "$1"))),
];
const PUBLIC = /^\/(authorize|invite|setup|sign-in|sign-up|signed-in-again|unsubscribe|auth\/|me\/email\/|share\/)/;
const viewsSource = readFileSync(new URL("../src/kaeso/overview/views.ts", import.meta.url), "utf8");
const views = (name: string) => [...(new RegExp(`${name} = \\[([^\\]]*)\\]`).exec(viewsSource)?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((m) => m[1] ?? "");
const VIEWS = { org: views("ORG_VIEWS"), project: views("PROJECT_VIEWS") };
const TABS: Record<string, string[]> = {
  "/orgs/$org/projects/$project/models": ["?tab=aliases", "?tab=retiring"],
  "/settings/orgs/$org/members": ["?tab=invites", "?tab=roles"],
  "/settings/orgs/$org/logging": ["?tab=storage", "?tab=deletions"],
  "/orgs/$org/providers": ["?tab=custom"],
  "/orgs/$org/projects/$project/tools": ["?tab=servers", "?tab=connect"],
  "/settings/instance/users": ["?tab=invites"],
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
      .replace("$instanceOrg", org)
      .replace("$org", org)
      .replace("$project", project)
      .replace("$view", view)
      .replace("$alias", "no-such-alias")
      .replace("$agent", "agt_00000000000000000000000000")
      .replace("$token", share)
      .replace(/\/\$$/, "/no-such-page");
  const each = route.includes("$view") ? (route.includes("$project") ? VIEWS.project : VIEWS.org) : [""];
  return each.flatMap((view) => [filled(view), ...(TABS[route] ?? []).map((tab) => filled(view) + tab)]);
}

const SIDEWAYS_SCROLLERS_AND_WHAT_STICKS_OUT = `(() => {
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

// A screen at a time, so lazy content gets drawn.
const SCROLL_TO_THE_END = `(async () => {
  for (const el of [document.scrollingElement, ...document.querySelectorAll("[data-frame-body]")]) {
    if (!el || !el.clientHeight) continue;
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

// Controls a finger has to hit, at least 44px each way (a tap target's ::after counts). Left out on purpose: fields
// and the buttons inside them, which keep their 32-36px (a larger area would cover the caret), and links in running text
// or a table cell, whose area would cover the lines next to them.
// Measured from the top of every scroller, so the whole page counts, not only its last screen.
const SMALL_TOUCH_TARGETS = `(() => {
  const small = [];
  for (const sc of [document.scrollingElement, ...document.querySelectorAll("[data-frame-body], [data-dialog-body]")]) if (sc) sc.scrollTop = 0;
  for (const el of document.querySelectorAll("button, a[href], [role=button], [role=tab], [role=switch], [role=checkbox], [role=radio], [role=menuitem], [role=option], select, summary")) {
    const r = el.getBoundingClientRect();
    if (r.width <= 2 || r.height <= 2 || !el.checkVisibility({ visibilityProperty: true }) || el.closest("[aria-hidden=true], [inert]")) continue;
    if (el.closest(".material-field")) continue;
    if (el.tagName === "A" && !el.matches("[class*=tap-target]") && (getComputedStyle(el).display === "inline" || el.closest("p, [role=cell], [role=gridcell], dd"))) continue;
    const after = getComputedStyle(el, "::after");
    const tap = after.content !== "none" && after.position === "absolute";
    const w = Math.max(r.width, tap ? parseFloat(after.width) || 0 : 0);
    const h = Math.max(r.height, tap ? parseFloat(after.height) || 0 : 0);
    if (w < 43.5 || h < 43.5) small.push(Math.round(w) + "x" + Math.round(h) + " <" + el.tagName.toLowerCase() + "> " + ((el.getAttribute("aria-label") || el.textContent || "").trim().slice(0, 40)));
  }
  return small;
})()`;

// The placeholders urls() fills in for a route's own ids, which answer with an error on purpose.
const EXPECTED = /\/(aliases\/no-such-alias|me\/agents\/agt_0{26})$/;
// A signed-out visit asks whether it is signed in; signed in, the same answer would mean the session dropped.
const SIGNED_OUT = /^\/platform\/v1\/(auth\/session|me\/memberships)$/;
// The SSO popup's last page closes itself; opened as a page of its own, the browser says it may not.
const EXPECTED_WARNING = /^Scripts may close only the windows that were opened by them\.$/;

async function check(page: Page, url: string, signedIn: boolean): Promise<void> {
  const problems: string[] = [];
  const onConsole = (m: { type(): string; text(): string }) => {
    if (m.type() === "warning" && !EXPECTED_WARNING.test(m.text())) problems.push(`console warning: ${m.text()}`);
  };
  const onResponse = (r: { status(): number; url(): string; request(): { method(): string } }) => {
    const path = new URL(r.url()).pathname;
    if (r.status() >= 400 && !EXPECTED.test(path) && (signedIn || !SIGNED_OUT.test(path)))
      problems.push(`${r.request().method()} ${new URL(r.url()).pathname} answered ${r.status()}`);
  };
  const onFailed = (r: { url(): string; failure(): { errorText: string } | null }) => {
    const reason = r.failure()?.errorText ?? "";
    // A request the page itself dropped by moving on is not a failure.
    if (!reason.includes("ERR_ABORTED")) problems.push(`${new URL(r.url()).pathname} failed: ${reason}`);
  };
  page.on("console", onConsole);
  page.on("response", onResponse);
  page.on("requestfailed", onFailed);
  try {
    await page.goto(url);
    // Counts the route's code too, which a development server compiles on its first visit.
    await page.waitForLoadState("networkidle");
    await settled(page);
    await page.evaluate(SCROLL_TO_THE_END);
    await settled(page);
    // Soft, so one run names every kind of problem a page has, not only the first.
    expect.soft(await page.evaluate<string[]>(SIDEWAYS_SCROLLERS_AND_WHAT_STICKS_OUT), url).toEqual([]);
    expect.soft(await page.evaluate<string[]>(SMALL_TOUCH_TARGETS), `${url}: controls smaller than a finger`).toEqual([]);
    const axe = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
    expect
      .soft(
        axe.violations
          .filter((v) => v.impact === "serious" || v.impact === "critical")
          .map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`),
        `${url}: serious accessibility problems`,
      )
      .toEqual([]);
    expect.soft(problems, `${url}: warnings and failed requests`).toEqual([]);
    if (signedIn) expect.soft(new URL(page.url()).pathname, `${url}: the session held`).not.toBe("/sign-in");
  } finally {
    page.off("console", onConsole);
    page.off("response", onResponse);
    page.off("requestfailed", onFailed);
  }
}

test("the route list holds every page of the console", () => {
  expect(ROUTES).toContain("/orgs/$org/projects/$project/logs");
  expect(ROUTES.length).toBeGreaterThan(60);
});

for (const route of ROUTES) {
  test(`${route} fits a phone: nothing scrolls sideways, every control takes a finger, axe finds nothing serious, and nothing fails or warns`, async ({
    page,
    browser,
  }) => {
    await page.context().addCookies(cookies);
    for (const url of urls(route, ids)) await check(page, url, !PUBLIC.test(route));
    if (!PUBLIC.test(route)) return;
    const signedOut = await browser.newContext(PHONE);
    try {
      const out = await signedOut.newPage();
      for (const url of urls(route, ids)) await check(out, url, false);
    } finally {
      await signedOut.close();
    }
  });
}
