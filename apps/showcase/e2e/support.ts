import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Page, Request } from "@playwright/test";

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
export const localizedPages = ["de", "zh-CN"].flatMap((lang) => [...overviewPages, "/components/button", "/404"].map((p) => (p === "/" ? `/${lang}` : `/${lang}${p}`)));
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

export async function hydrated(page: Page) {
  await page.waitForFunction(() => {
    const inView = (el: Element) => {
      const r = el.getBoundingClientRect();
      return r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth;
    };
    return [...document.querySelectorAll("astro-island[ssr]")].every((island) => island.getAttribute("client") === "visible" && ![...island.children].some(inView));
  });
}

interface Network {
  started: number;
  quiet: () => Promise<void>;
}
const networks = new WeakMap<Page, Network>();

function watchNetwork(page: Page): Network {
  const pending = new Set<Request>();
  let waiting: (() => void)[] = [];
  const network: Network = {
    started: 0,
    quiet: () => (pending.size === 0 ? Promise.resolve() : new Promise((resolve) => waiting.push(resolve))),
  };
  const done = (request: Request) => {
    pending.delete(request);
    if (pending.size > 0) return;
    for (const resolve of waiting) resolve();
    waiting = [];
  };
  page.on("request", (request) => {
    pending.add(request);
    network.started += 1;
  });
  page.on("requestfinished", done);
  page.on("requestfailed", done);
  networks.set(page, network);
  return network;
}

async function atRest(page: Page) {
  await page.evaluate(async () => {
    const frames = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const moving = () => document.getAnimations().filter((a) => a.playState === "running" && a.effect?.getComputedTiming().endTime !== Infinity);
    const transitioning = () => document.querySelector("[data-starting-style], [data-ending-style]");
    const idle = () => new Promise((resolve) => requestIdleCallback(resolve));
    await document.fonts.ready;
    await idle();
    await frames();
    while (moving().length > 0 || transitioning()) {
      await Promise.all(moving().map((a) => a.finished.catch(() => undefined)));
      await idle();
      await frames();
    }
  });
}

export async function settle(page: Page) {
  await hydrated(page);
  const network = networks.get(page) ?? watchNetwork(page);
  let seen: number;
  do {
    seen = network.started;
    await network.quiet();
    await atRest(page);
  } while (seen !== network.started);
}

export async function open(page: Page, path: string) {
  if (!networks.has(page)) watchNetwork(page);
  const response = await page.goto(path);
  if (!response || response.status() >= 400) throw new Error(`${path}: HTTP ${response?.status()}`);
  await settle(page);
}

export async function growToContent(page: Page, width: number) {
  const height = await page.evaluate(() => document.getElementById("k-panel")?.scrollHeight ?? document.body.scrollHeight);
  await page.setViewportSize({ width, height: Math.min(height, 16000) });
  await settle(page);
}

export async function nextFrame(page: Page) {
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}
