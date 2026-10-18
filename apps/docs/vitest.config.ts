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
      changelog: "e2e/fixtures/CHANGELOG.md",
      editBase: "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/",
      siteUrl: "https://docs.kaeso.app",
      version: kaesoVersion(root),
    }),
    react(),
  ],
  resolve: { alias: { "~": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: {
    css: false,
    projects: [
      {
        extends: true,
        test: {
          name: "site",
          environment: "happy-dom",
          environmentOptions: { happyDOM: { url: "http://localhost:4480/" } },
          include: ["src/**/*.test.{ts,tsx}"],
          setupFiles: ["./src/test/setup.ts"],
        },
      },
      { extends: true, test: { name: "build", environment: "node", include: ["scripts/**/*.test.ts", "eslint/**/*.test.js"] } },
    ],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}", "scripts/**/*.ts", "eslint/**/*.js"],
      exclude: ["**/*.test.{ts,tsx,js}", "src/test/**", "src/main.tsx", "src/content.d.ts", "scripts/content/mermaid-page.ts"],
      reporter: ["text", "text-summary", "json-summary", "html"],
      thresholds: { statements: 95.5, branches: 86.4, functions: 97.3, lines: 96.7 },
    },
  },
});
