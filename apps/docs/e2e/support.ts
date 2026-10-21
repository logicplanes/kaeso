import { test as base, expect } from "@playwright/test";
import { UPDATE_SERVICE } from "../src/site/update-service.ts";

const ART = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360"><rect width="640" height="360" fill="#1c1c22"/></svg>`;

export const test = base.extend<{ browserErrors: string[]; releaseArt: string[] }>({
  releaseArt: [
    async ({ context }, use) => {
      const asked: string[] = [];
      await context.route(`${UPDATE_SERVICE}/**`, (route) => {
        asked.push(route.request().url());
        return route.fulfill({ contentType: "image/svg+xml", body: ART });
      });
      await use(asked);
    },
    { auto: true },
  ],
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
