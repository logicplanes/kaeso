import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { hydrated } from "./support";

const read = (file: string) => readFileSync(new URL(`../dist/${file}`, import.meta.url), "utf8");
const policy = () => read("csp.txt").trim();

async function underPolicy(page: Page) {
  const header = policy();
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
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
  return async () => [...(await page.evaluate(() => (window as unknown as { __violations: string[] }).__violations)), ...errors];
}

test("the build writes one policy that allows its inline scripts by hash, never by 'unsafe-inline'", () => {
  const header = policy();
  const directives: Record<string, string[]> = Object.fromEntries(header.split("; ").map((d) => [d.split(" ")[0] ?? "", d.split(" ").slice(1)]));
  expect(directives["script-src"]?.[0]).toBe("'self'");
  expect(directives["script-src"]?.slice(1).every((s) => /^'sha256-[\w+/]+={0,2}'$/.test(s))).toBe(true);
  expect(directives["style-src"]?.slice(1).every((s) => /^'sha256-[\w+/]+={0,2}'$/.test(s))).toBe(true);
  expect(directives["style-src-attr"]).toEqual(["'unsafe-inline'"]);
  expect(directives["frame-ancestors"]).toEqual(["'none'"]);
  expect(header.replace("style-src-attr 'unsafe-inline'", "")).not.toContain("unsafe");
  expect(read("_headers")).toBe(`/*\n  Content-Security-Policy: ${header}\n`);
  expect(read("components/button.html")).toContain(`<meta http-equiv="content-security-policy" content="${header.replace("; frame-ancestors 'none'", "")}">`);
});

const THEME = { en: "Theme", de: "Design", "zh-CN": "主题" } as const;
const languageOf = (path: string) => (path.startsWith("/de") ? "de" : path.startsWith("/zh-CN") ? "zh-CN" : "en");
const pages = ["/", "/installation", "/changelog", "/components/select", "/components/scroll-area", "/components/conversation-view/states", "/de", "/zh-CN/components/dialog", "/404"];

for (const colorScheme of ["light", "dark"] as const) {
  for (const path of pages) {
    test(`${path} in the ${colorScheme} theme runs under the policy without a violation`, async ({ page }) => {
      await page.emulateMedia({ colorScheme });
      const violations = await underPolicy(page);
      await page.goto(path);
      await hydrated(page);
      if (!path.endsWith("/states")) {
        await page.getByRole("combobox", { name: THEME[languageOf(path)] }).click();
        await expect(page.getByRole("option").first()).toBeVisible();
        await page.keyboard.press("Escape");
      }
      expect(await violations()).toEqual([]);
    });
  }
}

test("moving between pages, languages and themes stays within the policy", async ({ page }) => {
  const violations = await underPolicy(page);
  await page.goto("/components/button");
  await hydrated(page);
  await page.getByRole("link", { name: "Select", exact: true }).first().click();
  await expect(page).toHaveURL("/components/select");
  await page.getByRole("combobox", { name: "Theme" }).click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("combobox", { name: "Language" }).click();
  await page.getByRole("option", { name: "Deutsch" }).click();
  await expect(page).toHaveURL("/de/components/select");
  await page.getByRole("link", { name: "Übersicht" }).first().click();
  await expect(page).toHaveURL("/de");
  expect(await violations()).toEqual([]);
});
