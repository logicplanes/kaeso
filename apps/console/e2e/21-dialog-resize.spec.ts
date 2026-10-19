import { createHmac } from "node:crypto";
import type { Locator, Page } from "@playwright/test";
import { expect, test, openDefaultProject, signInAsOwner } from "./support";

test.describe.configure({ mode: "serial" });
test.use({ reducedMotion: "no-preference" });

interface Sample {
  t: number;
  w: number;
  h: number;
  cx: number;
  cy: number;
  top: number;
}

interface Motion {
  samples: Sample[];
  duration: number;
  easing: string;
}

function totp(secret: string, at = Date.now()): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of secret.replace(/=+$/, "").toUpperCase()) bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
  const key = Buffer.from(bits.match(/.{8}/g)?.map((b) => parseInt(b, 2)) ?? []);
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(at / 30_000)));
  const hmac = createHmac("sha1", key).update(counter).digest();
  const offset = (hmac[hmac.length - 1] ?? 0) & 0xf;
  return ((hmac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).toString().padStart(6, "0");
}

interface Geometry {
  height: number;
  width: number;
  natural: number;
  cx: number;
  cy: number;
  top: number;
  bottom: number;
  fixed: string;
}

const IN_PAGE = `
window.kaesoResize = {
  frame: () => new Promise((resolve) => requestAnimationFrame(resolve)),
  async still() {
    const busy = () =>
      document.querySelector("[data-resizing], [data-starting-style], [data-ending-style]") ||
      document.getAnimations().some((a) => a.playState === "running" && a.effect?.getComputedTiming().endTime !== Infinity);
    await this.frame();
    while (busy()) await this.frame();
    await this.frame();
  },
  start() {
    const box = [...document.querySelectorAll("[role=dialog]")].find((d) => d.querySelector("[data-dialog-frame]"));
    const samples = [];
    this.samples = samples;
    this.duration = box ? parseFloat(getComputedStyle(box).getPropertyValue("--k-duration-dialog")) : 0;
    this.easing = box ? getComputedStyle(box).getPropertyValue("--k-ease-standard").trim() : "";
    this.recording = true;
    const painted = new MessageChannel();
    const sample = () => {
      if (!box?.isConnected || !this.recording) return;
      const r = box.getBoundingClientRect();
      samples.push({ t: Number(document.timeline.currentTime), w: r.width, h: r.height, cx: r.left + r.width / 2 - innerWidth / 2, cy: r.top + r.height / 2 - innerHeight / 2, top: r.top });
      requestAnimationFrame(() => painted.port2.postMessage(null));
    };
    painted.port1.onmessage = sample;
    sample();
  },
  stop() {
    this.recording = false;
    return { samples: this.samples, duration: this.duration, easing: this.easing };
  },
  geometry(box) {
    const frame = box.querySelector("[data-dialog-frame]");
    const body = frame?.querySelector(":scope > [data-dialog-body]");
    let content = 0;
    for (const part of frame?.children ?? []) content += part === body ? body.scrollHeight : part.getBoundingClientRect().height;
    const r = box.getBoundingClientRect();
    return {
      height: r.height,
      width: r.width,
      natural: Math.min(content, parseFloat(getComputedStyle(box).maxHeight)),
      cx: r.left + r.width / 2 - innerWidth / 2,
      cy: r.top + r.height / 2 - innerHeight / 2,
      top: r.top,
      bottom: innerHeight - r.bottom,
      fixed: box.style.height + box.style.width + box.style.minHeight + (frame?.style.height ?? "") + (frame?.style.minHeight ?? ""),
    };
  },
};
`;

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: IN_PAGE });
});

const still = (page: Page) => page.evaluate("window.kaesoResize.still()");

async function record(page: Page, act: () => Promise<unknown>): Promise<Motion> {
  await still(page);
  await page.evaluate("window.kaesoResize.start()");
  await act();
  await still(page);
  return (await page.evaluate("window.kaesoResize.stop()")) as Motion;
}

async function centredAtNaturalSize(dialog: Locator) {
  await expect(dialog).not.toHaveAttribute("data-resizing");
  const g = await dialog.evaluate((box) => (globalThis as unknown as { kaesoResize: { geometry(el: typeof box): Geometry } }).kaesoResize.geometry(box));
  expect(Math.abs(g.cy)).toBeLessThanOrEqual(1);
  expect(Math.abs(g.cx)).toBeLessThanOrEqual(1);
  expect(Math.abs(g.height - g.natural)).toBeLessThanOrEqual(1);
  expect(g.fixed).toBe("");
  return g;
}

const anchorTop = async (page: Page) => Number(await page.evaluate("Math.max(16, innerHeight * 0.12)"));

async function anchoredAtNaturalSize(page: Page, dialog: Locator) {
  await expect(dialog).not.toHaveAttribute("data-resizing");
  const g = await dialog.evaluate((box) => (globalThis as unknown as { kaesoResize: { geometry(el: typeof box): Geometry } }).kaesoResize.geometry(box));
  expect(Math.abs(g.top - (await anchorTop(page)))).toBeLessThanOrEqual(1);
  expect(Math.abs(g.cx)).toBeLessThanOrEqual(1);
  expect(g.bottom).toBeGreaterThanOrEqual(15);
  expect(Math.abs(g.height - g.natural)).toBeLessThanOrEqual(1);
  expect(g.fixed).toBe("");
  return g;
}

const traceOf = ({ samples }: Motion, key: "h" | "w") => samples.map((x) => `${Math.round(x.t)}:${Math.round(x[key])}`).join(" ");

function timeToReach(easing: string, progress: number) {
  const [x1 = 0, y1 = 0, x2 = 1, y2 = 1] = (easing.match(/-?[\d.]+/g) ?? []).map(Number);
  const bezier = (a: number, b: number, t: number) => 3 * (1 - t) ** 2 * t * a + 3 * (1 - t) * t ** 2 * b + t ** 3;
  for (let i = 0; i <= 1000; i++) if (bezier(y1, y2, i / 1000) >= progress) return bezier(x1, x2, i / 1000);
  return 1;
}

function travelTime({ samples }: Motion, key: "h" | "w") {
  const from = samples[0]?.[key] ?? 0;
  const to = samples.at(-1)?.[key] ?? 0;
  const left = samples.findLast((s) => Math.abs(s[key] - from) <= 1)?.t ?? 0;
  const arrived = samples.find((s) => Math.abs(s[key] - to) <= 1)?.t ?? 0;
  return arrived - left;
}

function animationTime({ samples, duration, easing }: Motion, key: "h" | "w") {
  const span = Math.abs((samples.at(-1)?.[key] ?? 0) - (samples[0]?.[key] ?? 0));
  return duration * timeToReach(easing, 1 - 1 / span);
}

function stepsBack({ samples }: Motion, key: "h" | "w") {
  const towards = Math.sign((samples.at(-1)?.[key] ?? 0) - (samples[0]?.[key] ?? 0));
  return samples.slice(1).filter((s, i) => (s[key] - (samples[i]?.[key] ?? 0)) * towards < -1).length;
}

function expectCentredAndSmooth(motion: Motion, key: "h" | "w" = "h") {
  for (const s of motion.samples) expect(Math.abs(s.cy)).toBeLessThanOrEqual(1);
  expectSmooth(motion, key);
}

function expectAnchoredAndSmooth(motion: Motion, top: number, key: "h" | "w" = "h") {
  for (const s of motion.samples) expect(Math.abs(s.top - top)).toBeLessThanOrEqual(1);
  expectSmooth(motion, key);
}

function expectSmooth(motion: Motion, key: "h" | "w") {
  const { samples, duration } = motion;
  expect(samples.length).toBeGreaterThan(3);
  for (const s of samples) expect(Math.abs(s.cx)).toBeLessThanOrEqual(1);
  const trace = traceOf(motion, key);
  expect(duration).toBeGreaterThan(0);
  expect(Math.abs((samples.at(-1)?.[key] ?? 0) - (samples[0]?.[key] ?? 0)), trace).toBeGreaterThan(8);
  expect(travelTime(motion, key), trace).toBeGreaterThanOrEqual(animationTime(motion, key) / 2);
  expect(stepsBack(motion, key), trace).toBe(0);
}

const moved = ({ samples }: Motion) => Math.abs((samples.at(-1)?.h ?? 0) - (samples[0]?.h ?? 0)) > 8;
const footerButton = (dialog: Locator, name: string) => dialog.locator("[data-dialog-footer]").getByRole("button", { name, exact: true });

for (const viewport of [
  { name: "desktop", width: 1440, height: 900 },
  { name: "phone", width: 375, height: 812 },
]) {
  for (const colorScheme of ["light", "dark"] as const) {
    test.describe(`${viewport.name}, ${colorScheme}`, () => {
      test.use({ colorScheme });

      test("creating a key: each step at its own height, the key shown once in the same dialog, with its top edge held throughout", async ({ page }) => {
        await signInAsOwner(page);
        await openDefaultProject(page);
        await page.setViewportSize({ width: viewport.width, height: viewport.height });
        await page.goto(`${new URL(page.url()).pathname}/keys`);
        await page
          .getByRole("button", { name: /^Create( key)?$/ })
          .first()
          .click();
        const dialog = page.getByRole("dialog", { name: "Create key" });
        await expect(dialog).toBeVisible();
        await dialog.getByLabel("Name").fill(`resize-${viewport.name}-${colorScheme}`);
        const first = await anchoredAtNaturalSize(page, dialog);
        const heights = [first.height];
        for (const heading of ["When does it expire?", "What can it call?"]) {
          const motion = await record(page, () => footerButton(dialog, "Continue").click());
          await expect(dialog.getByRole("heading", { name: heading })).toBeVisible();
          if (moved(motion)) expectAnchoredAndSmooth(motion, await anchorTop(page));
          heights.push((await anchoredAtNaturalSize(page, dialog)).height);
        }
        const back = await record(page, () => footerButton(dialog, "Back").click());
        const top = await anchorTop(page);
        expect(back.samples.every((s) => Math.abs(s.top - top) <= 1)).toBe(true);
        expect(Math.abs((await anchoredAtNaturalSize(page, dialog)).height - (heights[1] ?? 0))).toBeLessThanOrEqual(1);
        await footerButton(dialog, "Continue").click();
        await footerButton(dialog, "Continue").click();
        await expect(dialog.getByRole("heading", { name: "Models and limits" })).toBeVisible();
        const before = await anchoredAtNaturalSize(page, dialog);
        const reveal = page.getByRole("dialog", { name: "Key created" });
        const morph = await record(page, async () => {
          await footerButton(dialog, "Create key").click();
          await expect(reveal).toBeVisible();
        });
        expectAnchoredAndSmooth(morph, top, "h");
        if (viewport.width > 760) expectAnchoredAndSmooth(morph, top, "w");
        const after = await anchoredAtNaturalSize(page, reveal);
        expect(after.height).toBeGreaterThan(before.height);
        if (viewport.width > 760) expect(after.width).toBe(720);
        await footerButton(reveal, "Done").click();
        await page.getByRole("alertdialog", { name: "Did you copy the key?" }).getByRole("button", { name: "Close without copying" }).click();
        await expect(reveal).toBeHidden();
      });
    });
  }
}

test("a dialog that jumps to its new size in one frame fails the smoothness check", async ({ page }) => {
  await signInAsOwner(page);
  await openDefaultProject(page);
  await page.goto(`${new URL(page.url()).pathname}/keys`);
  await page
    .getByRole("button", { name: /^Create( key)?$/ })
    .first()
    .click();
  const dialog = page.getByRole("dialog", { name: "Create key" });
  await dialog.getByLabel("Name").fill("resize-without-motion");
  for (const heading of ["When does it expire?", "What can it call?", "Models and limits"]) {
    await footerButton(dialog, "Continue").click();
    await expect(dialog.getByRole("heading", { name: heading })).toBeVisible();
  }
  await page.emulateMedia({ reducedMotion: "reduce" });
  const reveal = page.getByRole("dialog", { name: "Key created" });
  const jump = await record(page, async () => {
    await footerButton(dialog, "Create key").click();
    await expect(reveal).toBeVisible();
  });
  expect(moved(jump)).toBe(true);
  expect(travelTime(jump, "h"), traceOf(jump, "h")).toBeLessThan(animationTime(jump, "h") / 2);
  const top = await anchorTop(page);
  expect(() => expectAnchoredAndSmooth(jump, top, "h")).toThrow();
  await anchoredAtNaturalSize(page, reveal);
  await footerButton(reveal, "Done").click();
  await page.getByRole("alertdialog", { name: "Did you copy the key?" }).getByRole("button", { name: "Close without copying" }).click();
  await expect(reveal).toBeHidden();
});

for (const [viewport, colorScheme] of [
  [{ width: 1440, height: 900 }, "light"],
  [{ width: 375, height: 812 }, "dark"],
] as const) {
  test(`two-factor setup turns into the recovery codes in the same centered dialog (${viewport.width}px, ${colorScheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    await signInAsOwner(page);
    await page.setViewportSize(viewport);
    await page.goto("/settings/account/security");
    await page.getByRole("button", { name: "Set up" }).click();
    const dialog = page.getByRole("dialog", { name: "Set up two-factor authentication" });
    await expect(dialog.getByRole("img", { name: "QR code to add Kaeso to an authenticator app" })).toBeVisible();
    await centredAtNaturalSize(dialog);
    const secret = (await dialog.locator("code").first().textContent()) ?? "";
    const motion = await record(page, async () => {
      await dialog.getByRole("textbox", { name: "Code from the app" }).fill(totp(secret));
      await expect(page.getByRole("dialog", { name: "Store your recovery codes" })).toBeVisible();
    });
    const codes = page.getByRole("dialog", { name: "Store your recovery codes" });
    for (const s of motion.samples) expect(Math.abs(s.cy)).toBeLessThanOrEqual(1);
    if (moved(motion)) expectCentredAndSmooth(motion);
    await centredAtNaturalSize(codes);
    await expect(codes.getByRole("checkbox", { name: "I stored these codes" })).toBeFocused();
    await codes.getByRole("checkbox", { name: "I stored these codes" }).click();
    await codes.getByRole("button", { name: "Done" }).click();
    await expect(codes).toBeHidden();
    await page.getByRole("button", { name: "Turn off" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Turn off" }).click();
    await expect(page.getByRole("button", { name: "Set up" })).toBeVisible();
  });
}
