import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { expect, test } from "./support";

const TITLE = "How a request is checked against its limits";
const policy = () => readFileSync(new URL("../dist-e2e/csp.txt", import.meta.url), "utf8").trim();

async function underPolicy(page: Page) {
  const header = policy();
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.defineProperty(window, "__violations", { value: seen });
    document.addEventListener("securitypolicyviolation", (e) => seen.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  await page.route("**/*", async (route) => {
    const response = await route.fetch();
    return route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": header } });
  });
  return () => page.evaluate(() => (window as unknown as { __violations: string[] }).__violations);
}

const shown = (page: Page, name: string) => page.getByRole("img", { name, exact: true });
const drawing = (page: Page, name: string) =>
  shown(page, name).evaluate((img: HTMLImageElement) => ({ src: img.currentSrc, loaded: img.complete && img.naturalWidth > 0 }));

for (const colorScheme of ["light", "dark"] as const) {
  test(`a diagram shows its ${colorScheme} drawing in the ${colorScheme} system theme, under the policy`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    const violations = await underPolicy(page);
    await page.goto("/guides/keys-and-limits");
    const region = page.getByRole("region", { name: `Diagram: ${TITLE}` });
    await region.scrollIntoViewIfNeeded();
    await expect(shown(page, TITLE)).toBeVisible();
    await expect(shown(page, TITLE)).toHaveCount(1);
    await expect.poll(() => drawing(page, TITLE)).toMatchObject({ src: expect.stringMatching(new RegExp(`/assets/[0-9a-f]{16}\\.${colorScheme}-[\\w-]+\\.svg$`)), loaded: true });
    await expect(shown(page, TITLE)).toHaveAccessibleDescription(/the gateway checks the key's, the member's and the project's limits in Valkey/);
    expect(await violations()).toEqual([]);
  });
}

test("a diagram follows the site's theme setting, not only the system's, and prints its light drawing", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/guides/provider-keys");
  const title = "Which provider key a request uses";
  await shown(page, title).scrollIntoViewIfNeeded();
  await expect.poll(() => drawing(page, title)).toMatchObject({ src: expect.stringContaining(".light-"), loaded: true });
  await page.getByRole("combobox", { name: "Theme" }).click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect.poll(() => drawing(page, title)).toMatchObject({ src: expect.stringContaining(".dark-"), loaded: true });
  await page.emulateMedia({ media: "print" });
  await expect.poll(() => drawing(page, title)).toMatchObject({ src: expect.stringContaining(".light-") });
});

test("the search finds a page by its diagram's words", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+k");
  const input = page.getByPlaceholder("Search the documentation");
  await expect(input).toBeFocused();
  await page.keyboard.type("or else");
  await expect(input).toHaveValue("or else");
  await expect(page.getByRole("option", { name: /Provider keys/ }).first()).toBeVisible();
});

test("a diagram's words are in every language", async ({ page }) => {
  for (const [path, region, name] of [
    ["/de/guides/provider-keys", "Diagramm: Welchen Provider-Schlüssel eine Anfrage nutzt", "Welchen Provider-Schlüssel eine Anfrage nutzt"],
    ["/zh-CN/guides/provider-keys", "图示：请求使用哪个提供商密钥", "请求使用哪个提供商密钥"],
  ] as const) {
    await page.goto(path);
    await expect(page.getByRole("region", { name: region })).toBeAttached();
    await expect(shown(page, name)).toBeAttached();
  }
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  for (const colorScheme of ["light", "dark"] as const) {
    test(`a wide diagram scrolls inside its own region, which the keyboard reaches, and the page never scrolls sideways, in the ${colorScheme} theme`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme });
      const violations = await underPolicy(page);
      await page.goto("/guides/keys-and-limits");
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(375);
      const region = page.getByRole("region", { name: `Diagram: ${TITLE}` });
      await region.scrollIntoViewIfNeeded();
      await expect(region).toHaveAttribute("tabindex", "0");
      const { inner, outer } = await region.evaluate((el) => ({ inner: el.scrollWidth, outer: el.clientWidth }));
      expect(inner).toBeGreaterThan(outer);
      const width = await shown(page, TITLE).evaluate((img) => img.getBoundingClientRect().width);
      expect(width).toBeGreaterThanOrEqual(440);
      await page.goto("/zh-CN/guides/provider-keys");
      const fits = page.getByRole("region", { name: "图示：请求使用哪个提供商密钥" });
      await fits.scrollIntoViewIfNeeded();
      expect(await fits.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(375);
      const results = await new AxeBuilder({ page }).include("figure").withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
      expect(results.violations).toEqual([]);
      expect(await violations()).toEqual([]);
    });
  }
});
