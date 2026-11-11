import { expect, test } from "@playwright/test";
import { componentPages, examplePages, growToContent, open, statesPages } from "./support";

// UTC+14: no build runs there, so a date the build formats in its own zone can never match the browser's.
test.use({ timezoneId: "Pacific/Kiritimati" });

const pages = [...componentPages, ...examplePages, ...statesPages, "/de/components/data-table", "/zh-CN/components/data-table"];

test.describe("hydrating in a time zone other than the build's", () => {
  for (const path of pages) {
    test(path, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await open(page, path);
      await growToContent(page, 1440);
      expect(errors).toEqual([]);
    });
  }
});
