import { defineConfig, devices } from "@playwright/test";
import { CONSOLE_PORT, CONSOLE_URL, SSO_CONSOLE_PORT, SSO_CONSOLE_URL, STACK_URLS } from "./e2e/stack";

const consoleServer = (port: number, url: string, gateway: string, cache: string) => ({
  command: `pnpm exec vite --config e2e/vite.config.ts --port ${port} --strictPort`,
  url,
  reuseExistingServer: false,
  env: { KAESO_GATEWAY_URL: gateway, KAESO_VITE_CACHE: cache },
  timeout: 120_000,
});

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
  webServer: [
    consoleServer(CONSOLE_PORT, CONSOLE_URL, STACK_URLS.gateway, "node_modules/.vite-e2e"),
    consoleServer(SSO_CONSOLE_PORT, SSO_CONSOLE_URL, STACK_URLS.ssoGateway, "node_modules/.vite-e2e-sso"),
  ],
});
