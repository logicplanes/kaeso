import { expect, test, type Page } from "@playwright/test";
import { open } from "./support";

const SIDEBAR = { en: "Overview", de: "Übersicht", "zh-CN": "概览" } as const;
const SOURCE = { en: "View source", de: "Quelltext ansehen", "zh-CN": "查看源码" } as const;
const PRIMARY = {
  de: ["Primär", "Tinte mit einem feinen Licht oben. Für die eine Aktion, für die eine Ansicht da ist."],
  "zh-CN": ["主要", "墨色，顶部带一抹微光。用于视图存在意义所在的那一个操作。"],
} as const;

function failures(page: Page) {
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(e.message));
  return errors;
}

test.describe("pages in every language", () => {
  for (const lang of ["de", "zh-CN"] as const) {
    test(`/${lang} is written in ${lang} before any script runs`, async ({ browser }) => {
      const context = await browser.newContext({ javaScriptEnabled: false, locale: "en-US" });
      const page = await context.newPage();
      await page.goto(`/${lang}/components/button`);
      await expect(page.locator("html")).toHaveAttribute("lang", lang);
      await expect(page.getByRole("link", { name: SIDEBAR[lang] }).first()).toBeVisible();
      await expect(page.getByRole("link", { name: SOURCE[lang] })).toBeVisible();
      await context.close();
    });

    test(`/${lang} names and describes every example in ${lang}`, async ({ page }) => {
      const [title, description] = PRIMARY[lang];
      await open(page, `/${lang}/components/button/examples`);
      const example = page.locator("#ex-variant-primary");
      await expect(example.getByRole("heading", { level: 2 })).toHaveText(title);
      await expect(example.getByText(description)).toBeVisible();
      await expect(page.getByRole("heading", { level: 2, name: "Primary", exact: true })).toHaveCount(0);
    });

    test(`/${lang} hydrates in ${lang} without a mismatch`, async ({ page }) => {
      const errors = failures(page);
      await open(page, `/${lang}/components/button`);
      await expect(page.getByRole("link", { name: SIDEBAR[lang] }).first()).toBeVisible();
      await page.getByRole("button", { name: lang === "de" ? "Sprache" : "语言" }).click();
      await expect(page.getByRole("menuitemradio", { name: lang === "de" ? "Deutsch" : "简体中文" })).toHaveAttribute("aria-checked", "true");
      expect(errors).toEqual([]);
    });
  }

  test("every localized page names its other languages", async ({ request }) => {
    const html = await (await request.get("/de/installation")).text();
    expect(html).toContain('<html lang="de"');
    for (const [lang, href] of [
      ["en", "https://ui.kaeso.app/installation"],
      ["de", "https://ui.kaeso.app/de/installation"],
      ["zh-CN", "https://ui.kaeso.app/zh-CN/installation"],
      ["x-default", "https://ui.kaeso.app/installation"],
    ])
      expect(html).toContain(`<link rel="alternate" hreflang="${lang}" href="${href}">`);
    expect(html).toContain('<link rel="canonical" href="https://ui.kaeso.app/de/installation">');
  });
});

test.describe("the visitor's language", () => {
  test("a first visit follows the browser's languages", async ({ browser }) => {
    const context = await browser.newContext({ locale: "de-DE" });
    const page = await context.newPage();
    await page.goto("/components/button");
    await expect(page).toHaveURL(/\/de\/components\/button$/);
    await expect(page.getByRole("link", { name: "Quelltext ansehen" })).toBeVisible();
    await context.close();
  });

  test("?lang= opens that language and is remembered", async ({ page }) => {
    await page.goto("/theming?lang=zh-CN#tokens");
    await expect(page).toHaveURL(/\/zh-CN\/theming#tokens$/);
    await page.goto("/");
    await expect(page).toHaveURL(/\/zh-CN$/);
  });

  test("a link to a language is followed, whatever was chosen before", async ({ page }) => {
    await page.goto("/?lang=de");
    await expect(page).toHaveURL(/\/de$/);
    await page.goto("/zh-CN/installation");
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
  });

  test("the language switch moves to the same page in the chosen language and remembers it", async ({ browser }) => {
    const context = await browser.newContext({ locale: "de-DE" });
    const page = await context.newPage();
    const errors = failures(page);
    await open(page, "/de/components/button");
    await page.getByRole("button", { name: "Sprache" }).click();
    await page.getByRole("menuitemradio", { name: "English" }).click();
    await expect(page).toHaveURL(/\/components\/button$/);
    await expect(page).not.toHaveURL(/\/de\//);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(page.getByRole("link", { name: "View source" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Overview" }).first()).toBeVisible();
    await page.goto("/installation");
    await expect(page).toHaveURL(/\/installation$/);
    await expect(page).not.toHaveURL(/\/de\//);
    expect(errors).toEqual([]);
    await context.close();
  });

  test("a missing page says so in the visitor's language", async ({ page }) => {
    await page.goto("/de/no-such-page");
    await expect(page).toHaveURL(/\/de\/404$/);
    await expect(page.getByRole("heading", { name: "Nicht gefunden" })).toBeVisible();
  });
});
