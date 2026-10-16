import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "~": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: {
    environment: "happy-dom",
    environmentOptions: { happyDOM: { url: "http://localhost:3000/" } },
    include: ["src/**/*.test.{ts,tsx}", "eslint/**/*.test.js", "scripts/**/*.test.mjs"],
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/test/**", "src/api/generated/**", "src/routeTree.gen.ts", "src/main.tsx"],
      reporter: ["text-summary", "json-summary", "html"],
      thresholds: { statements: 99.9, branches: 99.4, functions: 99.9, lines: 99.9 },
    },
  },
});
