import { test as base, expect } from "@playwright/test";

export const test = base.extend<{ browserErrors: string[] }>({
  browserErrors: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on("console", (m) => {
        if (m.type() === "error") errors.push(`console: ${m.text()}`);
      });
      page.on("pageerror", (e) => errors.push(`exception: ${e.message}`));
      await use(errors);
      expect(errors, "the browser reported errors").toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };
