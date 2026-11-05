import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { MAX_SECONDS, concatList, gifFilter, seconds } from "./demo-frames.mjs";
import { openTerminal, showCursor, writeTerminal } from "./demo-page.mjs";
import { STACKS } from "./stack.mjs";

const ORIGIN = STACKS.dev.publicUrl;
const ASSETS = join(dirname(fileURLToPath(import.meta.url)), "../../../.github/assets");
const VIEWPORT = { width: 1280, height: 800 };
const SCALE = 2;
const FPS = 20;
const HOLD_SECONDS = 1.5;
const OWNER = { email: "owner@example.com", password: "correct horse battery staple" };
const PROVIDER_SECRET = "sk-ant-mock-00000000000000anthropic";
const KEY_NAME = "checkout-api";
const MODEL = "claude-haiku-5-5";
const PROMPT = "Where is order 4471?";
const COMMAND = [
  "curl http://localhost:3000/v1/chat/completions \\",
  '  -H "Authorization: Bearer $KAESO_API_KEY" \\',
  `  --json '{"model": "${MODEL}", "messages": [{"role": "user", "content": "${PROMPT}"}]}' \\`,
  "  | jq '{model, tokens: .usage.total_tokens}'",
];

function api(request) {
  return async (method, path, data) => {
    const res = await request.fetch(`${ORIGIN}/platform/v1/${path}`, { method, headers: { origin: ORIGIN }, ...(data ? { data } : {}) });
    if (!res.ok()) throw new Error(`demo: ${method} ${path} answered ${res.status()} ${await res.text()}`);
    return res.status() === 204 ? null : res.json();
  };
}

async function prepare(request) {
  const call = api(request);
  await call("POST", "auth/password", OWNER);
  const home = (await call("GET", "me/memberships")).data[0];
  const project = home?.projects.find((p) => p.name === "Checkout assistant")?.id;
  if (!project) throw new Error("demo: no seeded organization; run `pnpm stack:up` and `pnpm seed` first");
  const org = home.org_id;
  for (const credential of (await call("GET", `orgs/${org}/credentials`)).data.filter((c) => c.provider === "anthropic"))
    await call("DELETE", `orgs/${org}/credentials/${credential.id}`);
  for (const key of (await call("GET", `orgs/${org}/projects/${project}/keys`)).data.filter((k) => k.name === KEY_NAME))
    await call("POST", `orgs/${org}/projects/${project}/keys/${key.id}/archive`);
  const { secret } = await call("POST", `orgs/${org}/projects/${project}/keys`, { name: KEY_NAME });
  return { org, project, secret };
}

function actor(page) {
  let at = { x: 140, y: 560 };
  const pause = (ms) => page.waitForTimeout(ms);
  const glide = async (to) => {
    const from = at;
    const steps = Math.round(Math.min(8, Math.max(4, Math.hypot(to.x - from.x, to.y - from.y) / 60)));
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const eased = t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
      await page.mouse.move(from.x + (to.x - from.x) * eased, from.y + (to.y - from.y) * eased);
      await pause(8);
    }
    at = to;
  };
  const rest = () => page.mouse.move(at.x, at.y);
  const click = async (locator, after = 350) => {
    await locator.waitFor();
    const box = await locator.boundingBox();
    await glide({ x: box.x + box.width / 2, y: box.y + box.height / 2 });
    await pause(60);
    await page.mouse.down();
    await pause(50);
    await page.mouse.up();
    await pause(after);
  };
  const type = async (text, after = 150) => {
    await page.keyboard.type(text, { delay: 26 });
    await pause(after);
  };
  return { pause, rest, glide, click, type };
}

async function terminal(page, secret, pause) {
  await page.evaluate(openTerminal);
  await pause(250);
  const write = (text, { dim = false, delay = 0 } = {}) => page.evaluate(writeTerminal, { text, dim, delay });
  await write("$ ", { dim: true });
  for (const [i, line] of COMMAND.entries()) await write(i ? `\n${line}` : line, { delay: 8 });
  await pause(200);
  const res = await fetch(`${ORIGIN}/v1/chat/completions`, {
    method: "POST",
    headers: { authorization: `Bearer ${secret}`, "content-type": "application/json" },
    body: JSON.stringify({ model: MODEL, messages: [{ role: "user", content: PROMPT }] }),
  });
  if (!res.ok) throw new Error(`demo: the request answered ${res.status} ${await res.text()}`);
  const { model, usage } = await res.json();
  await write(`\n${JSON.stringify({ model, tokens: usage.total_tokens }, null, 2)}\n`);
  await write("$ ", { dim: true });
}

async function record(browser, theme, out) {
  const context = await browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: SCALE, colorScheme: theme, baseURL: ORIGIN, locale: "en-US" });
  await context.addInitScript(showCursor);
  const { org, project, secret } = await prepare(context.request);
  const page = await context.newPage();
  for (const path of [`/orgs/${org}/providers`, `/orgs/${org}/projects/${project}/logs`, `/orgs/${org}`]) {
    await page.goto(path);
    await page.waitForLoadState("networkidle");
  }
  const { pause, rest, glide, click, type } = actor(page);
  await rest();
  const nav = page.getByRole("navigation", { name: "Console" });
  const frames = [];
  await page.screencast.start({
    size: { width: VIEWPORT.width * SCALE, height: VIEWPORT.height * SCALE },
    quality: 100,
    onFrame: ({ data, timestamp }) => {
      const file = `${String(frames.length).padStart(5, "0")}.jpg`;
      writeFileSync(join(out, file), data);
      frames.push({ file, time: timestamp / 1000 });
    },
  });
  await pause(300);

  await click(nav.getByRole("link", { name: "Providers" }), 300);
  await click(page.getByRole("button", { name: "Add provider key" }), 250);
  const connect = page.getByRole("dialog", { name: "Add provider key" });
  await click(connect.getByRole("radio", { name: /Anthropic/ }), 250);
  await type(PROVIDER_SECRET.slice(0, 12));
  await page.keyboard.insertText(PROVIDER_SECRET.slice(12));
  await click(connect.getByLabel("Name"), 100);
  await type("Production");
  await click(connect.getByRole("button", { name: "Add key", exact: true }), 350);

  await click(nav.getByRole("link", { name: "Checkout assistant" }), 200);
  await click(nav.getByRole("link", { name: "Request logs" }), 250);
  await click(page.getByRole("button", { name: "Live", exact: true }), 150);
  await glide({ x: 1180, y: 740 });
  await terminal(page, secret, pause);
  await page.locator("[data-fresh]").first().waitFor({ timeout: 15_000 });
  await pause(HOLD_SECONDS * 1000);

  await page.screencast.stop();
  await context.close();
  return frames;
}

function encode(frames, dir, gif) {
  writeFileSync(join(dir, "frames.txt"), concatList(frames, 1 / FPS));
  execFileSync(
    "ffmpeg",
    [
      "-v",
      "error",
      "-y",
      "-f",
      "concat",
      "-safe",
      "0",
      "-i",
      join(dir, "frames.txt"),
      "-filter_complex",
      gifFilter({ width: VIEWPORT.width, fps: FPS }),
      "-loop",
      "0",
      gif,
    ],
    { stdio: "inherit" },
  );
}

try {
  execFileSync("ffmpeg", ["-version"], { stdio: "ignore" });
} catch {
  throw new Error("demo: ffmpeg makes the GIFs; install it first, for example with `brew install ffmpeg`");
}

const browser = await chromium.launch();
try {
  for (const theme of ["light", "dark"]) {
    const dir = mkdtempSync(join(tmpdir(), `kaeso-demo-${theme}-`));
    try {
      const frames = await record(browser, theme, dir);
      const length = seconds(frames, 1 / FPS);
      if (length >= MAX_SECONDS) throw new Error(`demo: the ${theme} recording runs ${length.toFixed(1)} s; keep it under ${MAX_SECONDS} s`);
      const gif = join(ASSETS, `demo-${theme}.gif`);
      encode(frames, dir, gif);
      console.log(`demo: ${gif} (${(statSync(gif).size / 1024 / 1024).toFixed(1)} MB, ${length.toFixed(1)} s, ${frames.length} frames)`);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
} finally {
  await browser.close();
}
