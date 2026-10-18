import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import jsxA11y from "eslint-plugin-jsx-a11y";
import globals from "globals";
import kaeso from "./eslint/kaeso.js";

export default tseslint.config(
  { ignores: ["dist", "coverage", "node_modules", "src/api/generated", "src/routeTree.gen.ts", "test-results", "playwright-report"] },
  js.configs.recommended,
  ...tseslint.configs.strict,
  { rules: { "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }] } },
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks, "jsx-a11y": jsxA11y, kaeso },
    rules: {
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.flatConfigs.recommended.rules,
      "jsx-a11y/no-autofocus": ["error", { ignoreNonDOM: true }],
      "kaeso/layout-only": "error",
      "kaeso/no-color": "error",
      "kaeso/no-literal-text": "error",
      "no-console": ["error", { allow: ["error"] }],
    },
  },
  {
    files: ["src/**/*.test.{ts,tsx}", "src/test/**"],
    rules: { "kaeso/no-literal-text": "off" },
  },
  {
    files: ["scripts/**/*.mjs", "stack/**/*.mjs", "e2e/**/*.ts", "eslint/**/*.js", "*.config.{js,ts}"],
    languageOptions: { globals: globals.node },
  },
  {
    files: ["scripts/demo-page.mjs"],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ["e2e/**/*.spec.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "@playwright/test",
              importNames: ["test", "expect"],
              message: "Import test and expect from ./support: its test fails on the browser's errors.",
            },
          ],
        },
      ],
    },
  },
);
