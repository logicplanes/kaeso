import AxeBuilder from "@axe-core/playwright";
import { EDIT_BASE, FIXTURES } from "../playwright.config";
import { expect, test } from "./support";

test("the home page renders its title, description and text", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1, name: "Kaeso documentation" })).toBeVisible();
  await expect(page.getByText("Fixture pages for the end-to-end suite and the visual review.")).toBeVisible();
  await expect(page).toHaveTitle("Kaeso documentation");
  await expect(page.getByRole("navigation", { name: "Documentation" }).getByRole("link", { name: "Introduction" })).toHaveAttribute("aria-current", "page");
});

test("every page is built with its own title, description and alternates", async ({ request }) => {
  const html = await (await request.get("/guides/keys-and-limits")).text();
  expect(html).toContain("<title>Keys and limits · Kaeso documentation</title>");
  expect(html).toContain('<meta name="description" content="Create a key for an app and keep its spending inside a budget." />');
  expect(html).toContain('<link rel="canonical" href="https://docs.kaeso.app/guides/keys-and-limits" />');
  expect(html).toContain('<link rel="alternate" hreflang="de" href="https://docs.kaeso.app/de/guides/keys-and-limits" />');
  const german = await (await request.get("/de/guides/keys-and-limits")).text();
  expect(german).toContain('<html lang="de">');
  expect(german).toContain("<title>Schlüssel und Limits · Kaeso-Dokumentation</title>");
  const sitemap = await (await request.get("/sitemap.xml")).text();
  const english = [
    "/",
    "/guides",
    "/guides/keys-and-limits",
    "/guides/provider-keys",
    "/reference",
    "/reference/platform-api",
    "/reference/platform-api/keys",
    "/reference/models",
  ];
  const locs = [...sitemap.matchAll(/<loc>https:\/\/docs\.kaeso\.app([^<]*)<\/loc>/g)].map((m) => m[1] || "/");
  expect(locs.filter((l) => !/^\/(de|zh-CN)(\/|$)/.test(l))).toEqual(english);
  expect(locs).toHaveLength(english.length * 3);
});

test("the sidebar opens pages in place, with their title and contents", async ({ page }) => {
  await page.goto("/");
  const nav = page.getByRole("navigation", { name: "Documentation" });
  await nav.getByRole("link", { name: "Keys and limits" }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits");
  await expect(page.getByRole("heading", { level: 1, name: "Keys and limits" })).toBeVisible();
  await expect(page).toHaveTitle("Keys and limits · Kaeso documentation");
  await expect(nav.getByRole("link", { name: "Keys and limits" })).toHaveAttribute("aria-current", "page");
  const toc = page.getByRole("navigation", { name: "On this page" });
  await expect(toc.getByRole("link")).toHaveText(["Create a key", "From an SDK", "Limits", "Rate limits", "Budgets", "Checklist", "Notes"]);
  await expect(page.getByRole("heading", { name: "Keys and limits" })).toHaveCount(1);
});

test("a relative link between two Markdown files opens the page at its heading", async ({ page }) => {
  await page.goto("/guides/provider-keys");
  await page.getByRole("link", { name: "rate limits" }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits#rate-limits");
  await expect(page.getByRole("heading", { level: 3, name: "Rate limits" })).toBeInViewport();
});

test("a link to a heading opens there on first load too", async ({ page }) => {
  await page.goto("/guides/keys-and-limits#checklist");
  await expect(page.getByRole("heading", { level: 2, name: "Checklist" })).toBeInViewport();
});

test("previous and next follow the sidebar", async ({ page }) => {
  await page.goto("/guides/keys-and-limits");
  const steps = page.getByRole("navigation", { name: "Previous and next page" });
  await expect(steps.getByRole("link", { name: /Previous\s*Guides/ })).toHaveAttribute("href", "/guides");
  await steps.getByRole("link", { name: /Next\s*Provider keys/ }).click();
  await expect(page).toHaveURL("/guides/provider-keys");
});

test("Edit on GitHub opens the page's file", async ({ page }) => {
  await page.goto("/reference/platform-api/keys");
  const edit = page.getByRole("link", { name: /Edit on GitHub/ });
  await expect(edit).toHaveAttribute("href", `${EDIT_BASE}${FIXTURES}/en/reference/platform-api/keys.md`);
  await expect(edit).toHaveAttribute("target", "_blank");
});

test("search finds pages and headings, and says when nothing matches", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Search" }).click();
  const input = page.getByPlaceholder("Search the documentation");
  await expect(input).toBeFocused();
  await input.fill("rate");
  await expect(page.getByRole("option", { name: /Rate limits/ })).toBeVisible();
  await page.getByRole("option", { name: /Rate limits/ }).click();
  await expect(page).toHaveURL("/guides/keys-and-limits#rate-limits");
  await expect(page.getByRole("heading", { level: 3, name: "Rate limits" })).toBeInViewport();

  await page.keyboard.press("ControlOrMeta+k");
  await input.fill("zzzqx");
  await expect(page.getByText("Nothing matches. Try another word.")).toBeVisible();
  await input.fill("alias");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL("/reference/models");
});

test("what is typed right after ⌘K, before the search has loaded, becomes its query", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+k");
  await page.keyboard.type("alias");
  await expect(page.getByPlaceholder("Search the documentation")).toHaveValue("alias");
  await expect(page.getByRole("option", { name: /^Aliases/ })).toBeVisible();
});

test("the theme switches and stays switched", async ({ page }) => {
  await page.goto("/");
  const theme = page.getByRole("combobox", { name: "Theme" });
  await expect(theme).toHaveText("System");
  await theme.click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(theme).toHaveText("Dark");
  await theme.click();
  await page.getByRole("option", { name: "System" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme");
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the navigation is a sheet that closes when a page opens", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    await expect(page.getByRole("navigation", { name: "On this page" })).toBeHidden();
    const toggle = page.getByRole("button", { name: "Open navigation" });
    await toggle.click();
    const sheet = page.getByRole("dialog", { name: "Navigation" });
    await expect(sheet).toBeVisible();
    await expect(sheet.getByRole("combobox", { name: "Language" })).toHaveText("English");
    await expect(sheet.getByRole("combobox", { name: "Theme" })).toHaveText("System");
    await sheet.getByRole("link", { name: "Models" }).click();
    await expect(page).toHaveURL("/reference/models");
    await expect(sheet).toBeHidden();
    await expect(page.getByRole("heading", { level: 1, name: "Models" })).toBeVisible();
  });

  test("a wide table scrolls inside the page, which never scrolls sideways", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
    const region = page.locator("[aria-label=Table]");
    await expect(region).toHaveAttribute("tabindex", "0");
    await region.scrollIntoViewIfNeeded();
    const { inner, outer } = await region.evaluate((el) => ({ inner: el.scrollWidth, outer: el.clientWidth }));
    expect(inner).toBeGreaterThan(outer);
  });
});

test.describe("languages", () => {
  test("the language select opens the same page in another language, which the next visit remembers", async ({ page }) => {
    await page.goto("/guides/keys-and-limits");
    await expect(page.getByRole("combobox", { name: "Language" })).toHaveText("English");
    await page.getByRole("combobox", { name: "Language" }).click();
    await page.getByRole("option", { name: "Deutsch" }).click();
    await expect(page).toHaveURL("/de/guides/keys-and-limits");
    await expect(page.getByRole("heading", { level: 1, name: "Schlüssel und Limits" })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(page).toHaveTitle("Schlüssel und Limits · Kaeso-Dokumentation");
    await page.goto("/reference/models");
    await expect(page).toHaveURL("/de/reference/models");
    await expect(page.getByRole("combobox", { name: "Sprache" })).toHaveText("Deutsch");
    await page.getByRole("combobox", { name: "Sprache" }).click();
    await page.getByRole("option", { name: "简体中文" }).click();
    await expect(page).toHaveURL("/zh-CN/reference/models");
    await expect(page.getByRole("navigation", { name: "文档" })).toBeVisible();
  });

  test.describe("for a browser in German", () => {
    test.use({ locale: "de-DE" });

    test("the first visit follows the browser's language", async ({ page }) => {
      await page.goto("/guides/provider-keys");
      await expect(page).toHaveURL("/de/guides/provider-keys");
      await expect(page.getByRole("navigation", { name: "Dokumentation" }).getByRole("link", { name: "Anbieter-Schlüssel" })).toHaveAttribute(
        "aria-current",
        "page",
      );
    });

    test("an address with a query and a heading keeps both in German", async ({ page }) => {
      await page.goto("/guides/keys-and-limits?from=mail#limits");
      await expect(page).toHaveURL("/de/guides/keys-and-limits?from=mail#limits");
      await expect(page.getByRole("heading", { level: 2, name: "Limits" })).toBeInViewport();
    });
  });
});

test("an address without a page says so inside the frame", async ({ page }) => {
  await page.goto("/guides/nothing-here");
  await expect(page.getByText("This page doesn't exist.")).toBeVisible();
  await expect(page).toHaveTitle("Not found · Kaeso documentation");
  await page.getByRole("link", { name: "Go to the start" }).click();
  await expect(page).toHaveURL("/");
});

test.describe("accessibility", () => {
  for (const theme of ["light", "dark"] as const) {
    test(`pages in every language pass axe in the ${theme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
      for (const path of ["/", "/guides/keys-and-limits", "/de/reference/models", "/zh-CN/guides/provider-keys"]) {
        await page.goto(path);
        await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
        const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
        expect(results.violations.map((v) => `${path}: ${v.id} (${v.nodes.length})`)).toEqual([]);
      }
    });
  }
});
