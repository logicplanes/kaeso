import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    css: false,
    projects: [
      {
        extends: true,
        test: {
          name: "page",
          environment: "happy-dom",
          environmentOptions: { happyDOM: { url: "http://localhost:4490/" } },
          include: ["src/**/*.test.{ts,tsx}"],
          setupFiles: ["./src/test/setup.ts"],
        },
      },
      { extends: true, test: { name: "feed", environment: "node", include: ["scripts/**/*.test.ts", "eslint/**/*.test.js"] } },
    ],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}", "scripts/**/*.ts", "eslint/**/*.js"],
      exclude: ["**/*.test.{ts,tsx,js}", "src/test/**", "src/main.tsx"],
      reporter: ["text", "text-summary", "json-summary"],
      thresholds: { statements: 97, branches: 90, functions: 97, lines: 97 },
    },
  },
});
