import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import type { Browser } from "@playwright/test";
import { DIAGRAM_RADIUS, DIAGRAM_THEMES, mermaidConfig, type DiagramTheme } from "./diagram.ts";
import { loadFonts, renderInPage } from "./mermaid-page.ts";

export interface MermaidSource {
  key: string;
  code: string;
}

export type MermaidResult = { key: string; ok: true; svg: Record<DiagramTheme, string> } | { key: string; ok: false; line?: number; message: string };

const require = createRequire(import.meta.url);

export function interFonts(docsRoot: string): string[] {
  const dir = join(docsRoot, "node_modules", "@logicplanes", "kaeso-ui", "dist", "fonts");
  return ["inter-latin-opsz-normal.woff2", "inter-latin-ext-opsz-normal.woff2"].map((f) => join(dir, f));
}

const fontFace = (file: string) =>
  `@font-face{font-family:"Inter Variable";font-weight:100 900;src:url(data:font/woff2;base64,${readFileSync(file).toString("base64")}) format("woff2");}`;

export async function launchRenderer(): Promise<Browser> {
  const { chromium } = await import("@playwright/test");
  return chromium.launch();
}

export async function renderMermaid(sources: MermaidSource[], options: { fonts: string[]; browser?: Browser }): Promise<MermaidResult[]> {
  if (!sources.length) return [];
  const browser = options.browser ?? (await launchRenderer());
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
    await page.setContent("<!doctype html><html><head><meta charset=utf-8></head><body></body></html>");
    await page.addStyleTag({ content: options.fonts.map(fontFace).join("\n") });
    await page.addScriptTag({ path: require.resolve("mermaid/dist/mermaid.min.js") });
    await page.evaluate(loadFonts);
    const configs = Object.fromEntries(DIAGRAM_THEMES.map((t) => [t, mermaidConfig(t)])) as Record<DiagramTheme, Record<string, unknown>>;
    const results = await page.evaluate(renderInPage, { jobs: sources.map((s) => ({ key: s.key, code: s.code })), configs, radius: DIAGRAM_RADIUS });
    await page.close();
    return results as MermaidResult[];
  } finally {
    if (!options.browser) await browser.close();
  }
}
