import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "./support";

const GUIDE = "/guides/keys-and-limits";

test("a code group shows one language at a time, and the reader's choice follows them to the next page and the next visit", async ({ page }) => {
  await page.goto(GUIDE);
  const languages = page.getByRole("tablist", { name: "Language" });
  await expect(languages.getByRole("tab")).toHaveText(["curl", "Python", "TypeScript"]);
  await expect(languages.getByRole("tab", { name: "curl" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tabpanel", { name: "curl" })).toContainText("curl https://kaeso.example.com/v1/models");
  await languages.getByRole("tab", { name: "Python" }).click();
  await expect(page.getByRole("tabpanel", { name: "Python" })).toContainText("client.models.list()");
  await page.getByRole("link", { name: /^Models The names a key may call/ }).click();
  await expect(page).toHaveURL("/reference/models#aliases");
  await expect(page.getByRole("tab", { name: "Python" })).toHaveAttribute("aria-selected", "true");
  await page.goto(GUIDE);
  await expect(page.getByRole("tab", { name: "Python" })).toHaveAttribute("aria-selected", "true");
});

test("a heading links to itself, and following the link puts its address in the bar", async ({ page }) => {
  await page.goto(GUIDE);
  const heading = page.getByRole("heading", { level: 2, name: "Limits" });
  const link = heading.getByRole("link", { name: "Limits" });
  await expect(link).toHaveAttribute("href", "#limits");
  const mark = link.locator('[aria-hidden="true"]');
  await expect(mark).toHaveText("#");
  await expect(mark).toHaveCSS("opacity", "0");
  await link.hover();
  await expect(mark).toHaveCSS("opacity", "1");
  await link.click();
  await expect(page).toHaveURL(`${GUIDE}#limits`);
  await expect(heading).toBeInViewport();
});

test("next steps are cards that open their page", async ({ page }) => {
  await page.goto(GUIDE);
  const steps = page.getByRole("heading", { level: 2, name: "Next steps" }).locator("xpath=following-sibling::ul[1]");
  await expect(steps.getByRole("listitem")).toHaveCount(2);
  const card = steps.getByRole("link", { name: "Provider keys Connect a provider so the key can call its models." });
  await expect(card).toBeVisible();
  await card.click();
  await expect(page).toHaveURL("/guides/provider-keys");
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true, isMobile: true });

  test("code groups and next steps fit the column without the page scrolling sideways", async ({ page }) => {
    for (const path of [GUIDE, "/de/guides/keys-and-limits", "/zh-CN/guides/keys-and-limits"]) {
      await page.goto(path);
      await expect(page.getByRole("tablist").first()).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    }
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`code groups, heading links and next steps pass axe in every language in the ${theme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: theme });
    for (const path of [GUIDE, "/de/guides/keys-and-limits", "/zh-CN/guides/keys-and-limits"]) {
      await page.goto(path);
      await expect(page.getByRole("tablist").first()).toBeVisible();
      const results = await new AxeBuilder({ page }).include("article").withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
      expect(results.violations.map((v) => `${path}: ${v.id} (${v.nodes.length})`)).toEqual([]);
    }
  });
}
