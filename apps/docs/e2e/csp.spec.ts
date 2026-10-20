import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support";

const policy = () => readFileSync(new URL("../dist-e2e/csp.txt", import.meta.url), "utf8").trim();

async function underPolicy(page: Page) {
  const header = policy();
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.defineProperty(window, "__violations", { value: seen });
    document.addEventListener("securitypolicyviolation", (e) => seen.push(`${e.effectiveDirective} ${e.blockedURI} ${e.sample}`));
  });
  await page.route("**/*", async (route) => {
    if (route.request().resourceType() !== "document") return route.continue();
    const response = await route.fetch();
    return route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": header } });
  });
  return () => page.evaluate(() => (window as unknown as { __violations: string[] }).__violations);
}

test("the build writes one strict policy, with no inline scripts or styles to allow", () => {
  const header = policy();
  expect(header).toBe(
    "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
  );
  expect(readFileSync(new URL("../dist-e2e/_headers", import.meta.url), "utf8")).toBe(`/*\n  Content-Security-Policy: ${header}\n  Strict-Transport-Security: max-age=63072000; includeSubDomains\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n`);
  const html = readFileSync(new URL("../dist-e2e/guides/keys-and-limits.html", import.meta.url), "utf8");
  expect(html).toContain(`<meta http-equiv="content-security-policy" content="${header.replace("; frame-ancestors 'none'", "")}">`);
});

for (const colorScheme of ["light", "dark"] as const) {
  for (const path of ["/", "/guides/keys-and-limits", "/de/guides", "/zh-CN/reference/models", "/changelog", "/guides/nothing-here"]) {
    test(`${path} in the ${colorScheme} theme runs under the policy without a violation`, async ({ page }) => {
      await page.emulateMedia({ colorScheme });
      const violations = await underPolicy(page);
      const response = await page.goto(path);
      expect(response?.headers()["content-security-policy"]).toBe(policy());
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      await page.keyboard.press("ControlOrMeta+k");
      const search = page.getByRole("dialog").getByRole("combobox");
      await expect(search).toBeFocused();
      await page.keyboard.type("key");
      await expect(search).toHaveValue("key");
      await expect(page.getByRole("option").first()).toBeVisible();
      await page.keyboard.press("Escape");
      const language = page.locator("[role=combobox]").filter({ hasText: /English|Deutsch|简体中文/ });
      await language.click();
      await expect(page.getByRole("option", { name: "Deutsch" })).toBeVisible();
      await page.keyboard.press("Escape");
      expect(await violations()).toEqual([]);
    });
  }
}

test.describe("on a phone", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  for (const colorScheme of ["light", "dark"] as const) {
    test(`the navigation sheet, loaded when it first opens, stays within the policy in the ${colorScheme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme });
      const violations = await underPolicy(page);
      await page.goto("/de/guides/keys-and-limits");
      await page.getByRole("button", { name: "Navigation öffnen" }).click();
      const sheet = page.getByRole("dialog", { name: "Navigation" });
      await expect(sheet).toBeVisible();
      await sheet.getByRole("combobox", { name: "Sprache" }).click();
      await page.getByRole("option", { name: "简体中文" }).click();
      await expect(page).toHaveURL("/zh-CN/guides/keys-and-limits");
      await page.getByRole("button", { name: "打开导航" }).click();
      await expect(page.getByRole("dialog", { name: "导航" })).toBeVisible();
      expect(await violations()).toEqual([]);
    });
  }
});

test("moving between pages, languages and themes stays within the policy", async ({ page }) => {
  const violations = await underPolicy(page);
  await page.goto("/");
  await page.getByRole("link", { name: "Keys and limits" }).first().click();
  await expect(page).toHaveURL("/guides/keys-and-limits");
  const theme = page.getByRole("combobox", { name: "Theme" });
  await theme.click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("combobox", { name: "Language" }).click();
  await page.getByRole("option", { name: "简体中文" }).click();
  await expect(page).toHaveURL("/zh-CN/guides/keys-and-limits");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect(await violations()).toEqual([]);
});
