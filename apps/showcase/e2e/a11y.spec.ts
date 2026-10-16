import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { componentPages, examplePages, open, overviewPages, statesPages } from "./support";

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];
const pages = [...overviewPages, ...componentPages, ...examplePages, ...statesPages];

async function baseUiGuard(page: Page, target: string) {
  return page.evaluate((selector) => {
    const el = document.querySelector(selector);
    return !!el?.closest("[data-base-ui-inert], [data-base-ui-focus-guard]") || !!el?.querySelector("[data-base-ui-inert]");
  }, target);
}

for (const theme of ["light", "dark"] as const) {
  test.describe(`accessibility in ${theme}`, () => {
    test.use({ colorScheme: theme });
    for (const path of pages) {
      test(path, async ({ page }) => {
        await open(page, path);
        const { violations } = await new AxeBuilder({ page }).withTags(TAGS).disableRules(["region", "landmark-unique", "page-has-heading-one"]).analyze();
        const found: string[] = [];
        for (const v of violations) {
          if (path.endsWith("/states") && v.id === "color-contrast") continue;
          const nodes = [];
          for (const n of v.nodes) {
            const target = n.target.join(" ");
            if (v.id === "aria-hidden-focus" && (await baseUiGuard(page, target))) continue;
            nodes.push(target);
          }
          if (nodes.length) found.push(`${v.id} [${v.impact}]: ${v.help} — ${nodes.slice(0, 3).join(", ")}`);
        }
        expect(found).toEqual([]);
      });
    }
  });
}
