import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { kaesoVersion } from "./scripts/content/docs.ts";
import { kaesoDocs } from "./scripts/content/plugin.ts";

const root = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  plugins: [
    kaesoDocs({
      root,
      contentDir: "e2e/fixtures/content",
      editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
      siteUrl: "https://docs.kaeso.app",
      version: kaesoVersion(root),
    }),
    react(),
  ],
  resolve: { alias: { "~": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: {
    environment: "happy-dom",
    environmentOptions: { happyDOM: { url: "http://localhost:4480/" } },
    include: ["src/**/*.test.{ts,tsx}", "scripts/**/*.test.ts", "eslint/**/*.test.js"],
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}", "scripts/**/*.ts", "eslint/**/*.js"],
      exclude: ["**/*.test.{ts,tsx,js}", "src/test/**", "src/main.tsx", "src/content.d.ts"],
      reporter: ["text", "text-summary", "json-summary", "html"],
      thresholds: { statements: 93.9, branches: 84, functions: 96, lines: 95.2 },
    },
  },
});
