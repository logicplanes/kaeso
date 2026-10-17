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
    this.recording = true;
    const painted = new MessageChannel();
    const sample = () => {
      if (!box?.isConnected || !this.recording) return;
      const r = box.getBoundingClientRect();
      samples.push({ t: Number(document.timeline.currentTime), w: r.width, h: r.height, cx: r.left + r.width / 2 - innerWidth / 2, cy: r.top + r.height / 2 - innerHeight / 2 });
      requestAnimationFrame(() => painted.port2.postMessage(null));
    };
    painted.port1.onmessage = sample;
    sample();
  },
  stop() {
    this.recording = false;
    return this.samples;
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
      fixed: box.style.height + box.style.width + box.style.minHeight + (frame?.style.height ?? "") + (frame?.style.minHeight ?? ""),
    };
  },
};
`;

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: IN_PAGE });
});

const still = (page: Page) => page.evaluate("window.kaesoResize.still()");

async function record(page: Page, act: () => Promise<unknown>): Promise<Sample[]> {
  await still(page);
  await page.evaluate("window.kaesoResize.start()");
  await act();
  await still(page);
  return (await page.evaluate("window.kaesoResize.stop()")) as Sample[];
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

function expectCentredAndSmooth(samples: Sample[], key: "h" | "w" = "h") {
  expect(samples.length).toBeGreaterThan(3);
  for (const s of samples) {
    expect(Math.abs(s.cy)).toBeLessThanOrEqual(1);
    expect(Math.abs(s.cx)).toBeLessThanOrEqual(1);
  }
  const values = samples.map((s) => s[key]);
  const span = Math.max(...values) - Math.min(...values);
  expect(span).toBeGreaterThan(8);
  samples.slice(1).forEach((s, i) => {
    const previous = samples[i] ?? s;
    const trace = samples.map((x) => `${Math.round(x.t)}:${Math.round(x[key])}`).join(" ");
    expect(Math.abs(s[key] - previous[key]), trace).toBeLessThanOrEqual(span * Math.min(1, 0.25 + (s.t - previous.t) / 40) + 2);
  });
}

const moved = (samples: Sample[]) => Math.abs((samples.at(-1)?.h ?? 0) - (samples[0]?.h ?? 0)) > 8;
const footerButton = (dialog: Locator, name: string) => dialog.locator("[data-dialog-footer]").getByRole("button", { name, exact: true });

for (const viewport of [
  { name: "desktop", width: 1440, height: 900 },
  { name: "phone", width: 375, height: 812 },
]) {
  for (const colorScheme of ["light", "dark"] as const) {
    test.describe(`${viewport.name}, ${colorScheme}`, () => {
      test.use({ colorScheme });

      test("creating a key: each step at its own height, the key shown once in the same dialog, centered throughout", async ({ page }) => {
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
        const first = await centredAtNaturalSize(dialog);
        const heights = [first.height];
        for (const heading of ["When does it expire?", "What can it call?"]) {
          const samples = await record(page, () => footerButton(dialog, "Continue").click());
          await expect(dialog.getByRole("heading", { name: heading })).toBeVisible();
          if (moved(samples)) expectCentredAndSmooth(samples);
          heights.push((await centredAtNaturalSize(dialog)).height);
        }
        const back = await record(page, () => footerButton(dialog, "Back").click());
        expect(back.every((s) => Math.abs(s.cy) <= 1)).toBe(true);
        expect(Math.abs((await centredAtNaturalSize(dialog)).height - (heights[1] ?? 0))).toBeLessThanOrEqual(1);
        await footerButton(dialog, "Continue").click();
        await footerButton(dialog, "Continue").click();
        await expect(dialog.getByRole("heading", { name: "Models and limits" })).toBeVisible();
        const before = await centredAtNaturalSize(dialog);
        const reveal = page.getByRole("dialog", { name: "Key created" });
        const morph = await record(page, async () => {
          await footerButton(dialog, "Create key").click();
          await expect(reveal).toBeVisible();
        });
        expectCentredAndSmooth(morph, "h");
        if (viewport.width > 760) expectCentredAndSmooth(morph, "w");
        const after = await centredAtNaturalSize(reveal);
        expect(after.height).toBeGreaterThan(before.height);
        if (viewport.width > 760) expect(after.width).toBe(720);
        await footerButton(reveal, "Done").click();
        await page.getByRole("alertdialog", { name: "Did you copy the key?" }).getByRole("button", { name: "Close without copying" }).click();
        await expect(reveal).toBeHidden();
      });
    });
  }
}

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
    const samples = await record(page, async () => {
      await dialog.getByRole("textbox", { name: "Code from the app" }).fill(totp(secret));
      await expect(page.getByRole("dialog", { name: "Store your recovery codes" })).toBeVisible();
    });
    const codes = page.getByRole("dialog", { name: "Store your recovery codes" });
    for (const s of samples) expect(Math.abs(s.cy)).toBeLessThanOrEqual(1);
    if (moved(samples)) expectCentredAndSmooth(samples);
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
