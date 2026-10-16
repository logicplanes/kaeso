import { defineConfig, devices } from "@playwright/test";
import { CONSOLE_PORT, CONSOLE_URL, STACK_URLS } from "./e2e/stack";

export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  globalSetup: "./e2e/global-setup.ts",
  use: {
    baseURL: CONSOLE_URL,
    trace: "retain-on-failure",
    ...devices["Desktop Chrome"],
    viewport: { width: 1440, height: 900 },
  },
  webServer: {
    command: `pnpm exec vite --config e2e/vite.config.ts --port ${CONSOLE_PORT} --strictPort`,
    url: CONSOLE_URL,
    reuseExistingServer: false,
    env: { KAESO_GATEWAY_URL: STACK_URLS.gateway, KAESO_VITE_CACHE: "node_modules/.vite-e2e" },
    timeout: 120_000,
  },
});
