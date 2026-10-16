import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const components = `${dist}components`;
if (!existsSync(components)) throw new Error("Build the showcase first: pnpm build");

const slugs = readdirSync(components)
  .filter((f) => f.endsWith(".html"))
  .map((f) => f.slice(0, -".html".length))
  .sort();
const withPage = (name: string) =>
  readdirSync(components)
    .filter((d) => existsSync(`${components}/${d}/${name}.html`))
    .sort()
    .map((d) => `/components/${d}/${name}`);

export const overviewPages = ["/", "/installation", "/theming", "/for-agents", "/changelog"];
export const componentPages = slugs.map((s) => `/components/${s}`);
export const examplePages = withPage("examples");
export const statesPages = withPage("states");

export interface Tokens {
  control: Record<string, { height: number }>;
  row: Record<string, number>;
  radius: Record<string, string>;
  type: Record<string, { size: number; lineHeight: number }>;
  motion: { duration: Record<string, number>; skeleton: { duration: number }; spinner: { turn: number } };
}
export const tokens = JSON.parse(readFileSync(fileURLToPath(import.meta.resolve("@logicplanes/kaeso-tokens/tokens.json")), "utf8")) as Tokens;

export async function settle(page: Page) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(250);
}

export async function open(page: Page, path: string) {
  const response = await page.goto(path, { waitUntil: "networkidle" });
  if (!response || response.status() >= 400) throw new Error(`${path}: HTTP ${response?.status()}`);
  await settle(page);
}

export async function growToContent(page: Page, width: number) {
  const height = await page.evaluate(() => document.getElementById("k-panel")?.scrollHeight ?? document.body.scrollHeight);
  await page.setViewportSize({ width, height: Math.min(height, 16000) });
  await page.waitForTimeout(500);
}
