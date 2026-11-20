import type { Browser, Page } from "@playwright/test";
import { OWNER, expect, signInAsOwner, test } from "./support";

test.describe.configure({ mode: "serial" });

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

/**
 * What makes the page scroll sideways: the document itself, a page-sized scroller (the frame's body) that is wider
 * than the screen, or an element past the right edge that no scrolling parent holds. Tables, code and the controls
 * that scroll on purpose do it inside their own box, so they don't count.
 */
function sideways(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const width = window.innerWidth;
    const found: string[] = [];
    const name = (el: Element) => `${el.tagName.toLowerCase()} "${(el.textContent ?? "").trim().replace(/\s+/g, " ").slice(0, 40)}"`;
    if (document.documentElement.scrollWidth > width) found.push(`the page is ${document.documentElement.scrollWidth}px wide`);
    for (const el of document.querySelectorAll("body *")) {
      const style = getComputedStyle(el);
      const box = el.getBoundingClientRect();
      if (!box.width || !box.height || style.display === "none") continue;
      const scrollsDown = style.overflowY === "auto" || style.overflowY === "scroll";
      if (scrollsDown && box.width >= width * 0.8 && el.scrollWidth > el.clientWidth + 1 && !el.closest("[role=grid], [role=table], table, pre, [role=tablist], [role=radiogroup]"))
        found.push(`${name(el)} scrolls sideways by ${el.scrollWidth - el.clientWidth}px`);
      if (box.right <= width + 1 || style.position === "fixed") continue;
      let held = false;
      for (let up = el.parentElement; up && !held; up = up.parentElement) {
        held = /(auto|scroll|hidden|clip)/.test(getComputedStyle(up).overflowX) && up.getBoundingClientRect().right <= width + 1;
      }
      if (!held) found.push(`${name(el)} ends at ${Math.round(box.right)}px`);
    }
    return found;
  });
}

/** Opens a page, scrolls every scroller to its foot so what renders late is measured too, then measures. */
async function fits(page: Page, path: string): Promise<void> {
  await page.goto(path);
  await expect(page.getByRole("heading", { level: 1 }).first()).toBeVisible();
  await expect(page.locator("[aria-busy=true]")).toHaveCount(0, { timeout: 20_000 });
  await page.evaluate(() => {
    for (const el of document.querySelectorAll<HTMLElement>("*")) {
      const style = getComputedStyle(el).overflowY;
      if ((style === "auto" || style === "scroll") && el.scrollHeight > el.clientHeight) el.scrollTop = el.scrollHeight;
    }
  });
  expect(await sideways(page), path).toEqual([]);
}

test("the organization's pages, its settings, the account and the instance fit a phone without scrolling sideways", async ({ browser, page }) => {
  await signInAsOwner(page);
  const memberships = (await (await page.request.get("/platform/v1/me/memberships")).json()) as { data: { org_id: string; org_name: string }[] };
  const org = memberships.data.find((m) => m.org_name === OWNER.org)?.org_id ?? memberships.data[0]?.org_id ?? "";
  const headers = { origin: new URL(page.url()).origin };
  // Long names, so a line that can't hold its text has to end in an ellipsis or wrap.
  await page.request.post(`/platform/v1/orgs/${org}/credentials`, {
    headers,
    data: { provider: "openai", name: "Research and evaluation team, shared batch workloads", secret: "sk-proj-mock-0000000000000phonepages" },
  });
  await page.request.post(`/platform/v1/orgs/${org}/platform-keys`, {
    headers,
    data: { name: "terraform-provisioning-pipeline-for-production-and-staging", expires_in_days: 30 },
  });

  const phone = await onPhone(browser, page);
  for (const path of [
    `/orgs/${org}`,
    `/orgs/${org}/projects`,
    `/orgs/${org}/providers`,
    `/orgs/${org}/providers?tab=custom`,
    `/orgs/${org}/mcp-servers`,
    `/settings/orgs/${org}/general`,
    `/settings/orgs/${org}/members`,
    `/settings/orgs/${org}/members?tab=invites`,
    `/settings/orgs/${org}/members?tab=roles`,
    `/settings/orgs/${org}/security`,
    `/settings/orgs/${org}/guardrails`,
    `/settings/orgs/${org}/logging`,
    `/settings/orgs/${org}/logging?tab=storage`,
    `/settings/orgs/${org}/logging?tab=deletions`,
    `/settings/orgs/${org}/webhooks`,
    `/settings/orgs/${org}/shared-links`,
    `/settings/orgs/${org}/platform-keys`,
    `/settings/orgs/${org}/audit-log`,
    "/settings/account/profile",
    "/settings/account/security",
    "/settings/account/sessions",
    "/settings/account/keys",
    "/settings/account/memberships",
    "/settings/account/notifications",
    "/settings/account/appearance",
    "/settings/account/connections",
    "/settings/account/agents",
    "/settings/instance/general",
    "/settings/instance/users",
    "/settings/instance/orgs",
    `/settings/instance/orgs/${org}`,
    "/settings/instance/logging",
    "/settings/instance/system",
    "/settings/instance/updates",
    "/settings/instance/audit-log",
  ])
    await fits(phone, path);

  await phone.goto(`/settings/orgs/${org}/members?tab=roles&role=org:admin`);
  const sheet = phone.getByRole("dialog", { name: "Admin" });
  await expect(sheet).toBeVisible();
  // The sheet's actions stay in its footer, on screen while the rights scroll under them.
  await expect(sheet.locator("[data-sheet-footer]").getByRole("button", { name: "Duplicate" })).toBeInViewport();
  expect(await sideways(phone)).toEqual([]);
  await phone.context().close();
});
