import { defineConfig, devices } from "@playwright/test";

const PORT = 4481;
export const FIXTURES = "e2e/fixtures/content";
export const EDIT_BASE = "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/";

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  retries: 0,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "retain-on-failure",
    ...devices["Desktop Chrome"],
    viewport: { width: 1440, height: 900 },
  },
  webServer: {
    command: `pnpm exec vite build --outDir dist-e2e --emptyOutDir && pnpm exec vite preview --outDir dist-e2e --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: false,
    env: { KAESO_DOCS_CONTENT: FIXTURES, KAESO_DOCS_PORT: String(PORT), KAESO_VITE_CACHE: "node_modules/.vite-e2e" },
    timeout: 120_000,
  },
});
