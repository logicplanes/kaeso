import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { expect, test } from "./support";

const COMPOSE = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../../../deploy/compose.yaml"), "utf8");
const ORIGIN = "http://kaeso.test";

function config(name: string): string {
  const start = COMPOSE.indexOf(`\n  ${name}:\n    content: |\n`);
  const lines = COMPOSE.slice(start).split("\n").slice(3);
  const body: string[] = [];
  for (const line of lines) {
    if (line === "") body.push("");
    else if (line.startsWith("      ")) body.push(line.slice(6));
    else break;
  }
  return `${body.join("\n").replace(/\n+$/, "")}\n`;
}

const PAGE = config("updating_page");
const CSP = /header Content-Security-Policy "([^"]+)"/.exec(config("caddyfile"))?.[1] ?? "";

async function serve(page: Page, maintenance: string | null, back = false): Promise<void> {
  await page.route(`${ORIGIN}/**`, async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === "/kaeso-maintenance.json") {
      return maintenance ? route.fulfill({ status: 200, contentType: "application/json", body: maintenance }) : route.fulfill({ status: 404, body: "" });
    }
    if (back) {
      return route.fulfill({ status: 200, contentType: "text/html", body: "<!doctype html><title>Kaeso</title>" });
    }
    return route.fulfill({ status: 503, contentType: "text/html; charset=utf-8", headers: { "content-security-policy": CSP, "retry-after": "30" }, body: PAGE });
  });
}

const UPDATING = JSON.stringify({ state: "updating", step: "migrating" });

for (const [locale, title, step] of [
  ["en-US", "Kaeso is being updated", "Updating the database"],
  ["de-DE", "Kaeso wird aktualisiert", "Aktualisierung der Datenbank"],
  ["zh-CN", "Kaeso 正在更新", "更新数据库"],
] as const) {
  for (const scheme of ["light", "dark"] as const) {
    test(`updating page: ${locale} ${scheme} shows the step and passes axe`, async ({ browser }) => {
      const context = await browser.newContext({ locale, colorScheme: scheme, viewport: { width: 1280, height: 800 } });
      const page = await context.newPage();
      await serve(page, UPDATING);
      await page.goto(`${ORIGIN}/settings/instance`);
      await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
      await expect(page.getByText(step, { exact: true })).toBeVisible();
      await expect(page.locator("html")).toHaveAttribute("lang", locale.startsWith("zh") ? "zh-CN" : locale.slice(0, 2));
      const axe = await new AxeBuilder({ page }).analyze();
      expect(axe.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
      await context.close();
    });
  }
}

test("updating page: without an update it says Kaeso is away, and returns when it's back", async ({ browser }) => {
  const context = await browser.newContext({ locale: "en-US", viewport: { width: 375, height: 812 } });
  const page = await context.newPage();
  await serve(page, null);
  await page.goto(`${ORIGIN}/orgs`);
  await expect(page.getByRole("heading", { level: 1, name: "Kaeso will be back in a moment" })).toBeVisible();
  await expect(page.getByRole("list", { name: "Progress" })).toBeHidden();
  const scroll = await page.evaluate(() => {
    const g = globalThis as unknown as { innerWidth: number; document: { documentElement: { scrollWidth: number } } };
    return g.document.documentElement.scrollWidth <= g.innerWidth;
  });
  expect(scroll).toBe(true);
  const axe = await new AxeBuilder({ page }).analyze();
  expect(axe.violations.map((v) => v.id)).toEqual([]);
  await page.unroute(`${ORIGIN}/**`);
  await serve(page, null, true);
  await page.waitForURL(`${ORIGIN}/orgs`, { waitUntil: "load" });
  await expect(page).toHaveTitle("Kaeso", { timeout: 15_000 });
  await context.close();
});
