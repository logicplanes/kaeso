import { readFileSync } from "node:fs";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { hydrated, open } from "./support";

test.use({ reducedMotion: "no-preference" });

const policy = readFileSync(new URL("../dist/csp.txt", import.meta.url), "utf8").trim();

test.beforeEach(async ({ page }) => {
  await page.route("**/*", async (route) => {
    if (route.request().resourceType() !== "document") return route.continue();
    const response = await route.fetch();
    return route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": policy } });
  });
  await page.addInitScript(() => {
    const w = window as unknown as { reported: string[] };
    w.reported = [];
    addEventListener("error", (e) => w.reported.push(e.message));
    document.addEventListener("securitypolicyviolation", (e) => w.reported.push(`${e.effectiveDirective} ${e.blockedURI} ${e.sample}`));
  });
});
test.afterEach(async ({ page }) => {
  expect(await page.evaluate(() => (window as unknown as { reported: string[] }).reported)).toEqual([]);
});

interface Sample {
  t: number;
  w: number;
  h: number;
  cx: number;
  cy: number;
  top: number;
}

const popup = (page: Page) => page.locator("[role=dialog]").filter({ has: page.locator("[data-dialog-frame]") });

async function openExample(page: Page, id: string) {
  await open(page, "/components/dialog/examples");
  const trigger = page.locator(`#ex-${id} .pv-stage button`).first();
  await trigger.scrollIntoViewIfNeeded();
  await hydrated(page);
  await trigger.click();
  const dialog = popup(page);
  await expect(dialog).toBeVisible();
  await still(page);
  return dialog;
}

async function still(page: Page) {
  await page.evaluate(async () => {
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
    const busy = () =>
      document.querySelector("[data-resizing], [data-starting-style], [data-ending-style]") ||
      document.getAnimations().some((a) => a.playState === "running" && a.effect?.getComputedTiming().endTime !== Infinity);
    await frame();
    while (busy()) await frame();
    await frame();
  });
}

async function record(page: Page, act: () => Promise<void>): Promise<Sample[]> {
  await page.evaluate(() => {
    const w = window as unknown as { samples: Sample[]; recording: boolean };
    const box = [...document.querySelectorAll<HTMLElement>("[role=dialog]")].find((d) => d.querySelector("[data-dialog-frame]"));
    w.samples = [];
    w.recording = true;
    const painted = new MessageChannel();
    const sample = () => {
      if (!box || !w.recording) return;
      const r = box.getBoundingClientRect();
      w.samples.push({ t: Number(document.timeline.currentTime), w: r.width, h: r.height, cx: r.left + r.width / 2 - innerWidth / 2, cy: r.top + r.height / 2 - innerHeight / 2, top: r.top });
      requestAnimationFrame(() => painted.port2.postMessage(null));
    };
    painted.port1.onmessage = sample;
    sample();
  });
  await act();
  await still(page);
  return page.evaluate(() => {
    const w = window as unknown as { samples: Sample[]; recording: boolean };
    w.recording = false;
    return w.samples;
  });
}

async function geometry(dialog: Locator) {
  return dialog.evaluate((box) => {
    const frame = box.querySelector<HTMLElement>("[data-dialog-frame]")!;
    const body = frame.querySelector<HTMLElement>(":scope > [data-dialog-body]");
    let content = 0;
    for (const part of frame.children) content += part === body ? body.scrollHeight : part.getBoundingClientRect().height;
    const r = box.getBoundingClientRect();
    return {
      height: r.height,
      width: r.width,
      natural: Math.min(content, parseFloat(getComputedStyle(box).maxHeight)),
      cx: r.left + r.width / 2 - innerWidth / 2,
      cy: r.top + r.height / 2 - innerHeight / 2,
      top: r.top,
      bottom: innerHeight - r.bottom,
      inline: box.style.height + box.style.width + box.style.minHeight + frame.style.height + frame.style.minHeight,
      scrolled: scrollY,
    };
  });
}

async function centredAtNaturalSize(dialog: Locator) {
  const g = await geometry(dialog);
  expect(Math.abs(g.cy)).toBeLessThanOrEqual(1);
  expect(Math.abs(g.cx)).toBeLessThanOrEqual(1);
  expect(Math.abs(g.height - g.natural)).toBeLessThanOrEqual(1);
  expect(g.inline).toBe("");
  await expect(dialog).not.toHaveAttribute("data-resizing");
  return g;
}

const anchorTop = (page: Page) => page.evaluate(() => Math.max(16, innerHeight * 0.12));

async function anchoredAtNaturalSize(page: Page, dialog: Locator) {
  const g = await geometry(dialog);
  expect(Math.abs(g.top - (await anchorTop(page)))).toBeLessThanOrEqual(1);
  expect(Math.abs(g.cx)).toBeLessThanOrEqual(1);
  expect(g.bottom).toBeGreaterThanOrEqual(15);
  expect(Math.abs(g.height - g.natural)).toBeLessThanOrEqual(1);
  expect(g.inline).toBe("");
  await expect(dialog).not.toHaveAttribute("data-resizing");
  return g;
}

async function expectAnchoredThroughout(page: Page, samples: Sample[]) {
  const top = await anchorTop(page);
  expect(samples.length).toBeGreaterThan(3);
  for (const s of samples) {
    expect(Math.abs(s.top - top)).toBeLessThanOrEqual(1);
    expect(Math.abs(s.cx)).toBeLessThanOrEqual(1);
  }
}

function expectCentredThroughout(samples: Sample[]) {
  expect(samples.length).toBeGreaterThan(3);
  for (const s of samples) {
    expect(Math.abs(s.cy)).toBeLessThanOrEqual(1);
    expect(Math.abs(s.cx)).toBeLessThanOrEqual(1);
  }
}

function expectSmooth(samples: Sample[], key: "h" | "w", distance?: number) {
  const values = samples.map((s) => s[key]);
  const span = distance ?? Math.max(...values) - Math.min(...values);
  expect(span).toBeGreaterThan(8);
  const trace = samples.map((x) => `${Math.round(x.t)}:${Math.round(x[key])}`).join(" ");
  let base = samples[0];
  let last = samples[0];
  for (const s of samples.slice(1)) {
    if (!base || !last) break;
    if (s.t > last.t) base = last;
    expect(Math.abs(s[key] - base[key]), trace).toBeLessThanOrEqual(span * Math.min(1, 0.25 + Math.max(s.t - base.t, 1000 / 60) / 40) + 2);
    last = s;
  }
  expect(new Set(values.map(Math.round)).size).toBeGreaterThan(3);
}

const press = (dialog: Locator, name: string) => dialog.locator("[data-dialog-footer]").getByRole("button", { name, exact: true }).click();

for (const viewport of [
  { name: "desktop", width: 1440, height: 900 },
  { name: "phone", width: 375, height: 812 },
]) {
  for (const colorScheme of ["light", "dark"] as const) {
    test.describe(`${viewport.name}, ${colorScheme}`, () => {
      test.use({ viewport: { width: viewport.width, height: viewport.height }, colorScheme });

      test("a step dialog keeps its top edge while it grows to a taller step and shrinks back to a shorter one", async ({ page }) => {
        const dialog = await openExample(page, "step-heights");
        const first = await anchoredAtNaturalSize(page, dialog);
        const grow = await record(page, () => press(dialog, "Continue"));
        await expectAnchoredThroughout(page, grow);
        expectSmooth(grow, "h");
        const second = await anchoredAtNaturalSize(page, dialog);
        expect(second.height).toBeGreaterThan(first.height + 40);
        const shrink = await record(page, () => press(dialog, "Back"));
        await expectAnchoredThroughout(page, shrink);
        expectSmooth(shrink, "h");
        const back = await anchoredAtNaturalSize(page, dialog);
        expect(Math.abs(back.height - first.height)).toBeLessThanOrEqual(1);
      });

      test("a grow interrupted by a shrink turns around from the size it reached, without a jump", async ({ page }) => {
        const dialog = await openExample(page, "step-heights");
        const first = await anchoredAtNaturalSize(page, dialog);
        await record(page, () => press(dialog, "Continue"));
        const second = await anchoredAtNaturalSize(page, dialog);
        await record(page, () => press(dialog, "Back"));
        const samples = await record(page, () =>
          dialog.evaluate(async (box) => {
            const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
            const button = (name: string) => [...box.querySelectorAll<HTMLElement>("[data-dialog-footer] button")].find((b) => b.innerText.trim() === name)!;
            button("Continue").click();
            for (let i = 0; i < 4; i++) await frame();
            button("Back").click();
            for (let i = 0; i < 2; i++) await frame();
            button("Continue").click();
            for (let i = 0; i < 3; i++) await frame();
            button("Back").click();
          }),
        );
        await expectAnchoredThroughout(page, samples);
        expectSmooth(samples, "h", second.height - first.height);
        const end = await anchoredAtNaturalSize(page, dialog);
        expect(Math.abs(end.height - first.height)).toBeLessThanOrEqual(1);
      });

      test("a step taller than the screen keeps the header and footer on screen and scrolls its body, not the page", async ({ page }) => {
        const dialog = await openExample(page, "step-heights");
        await record(page, () => press(dialog, "Continue"));
        await record(page, () => press(dialog, "Continue"));
        const tall = await anchoredAtNaturalSize(page, dialog);
        expect(tall.bottom).toBeGreaterThanOrEqual(15);
        await expect(dialog.getByRole("heading", { name: "Add limit" })).toBeInViewport();
        await expect(dialog.locator("[data-dialog-footer]")).toBeInViewport();
        const body = dialog.locator("[data-dialog-body]");
        expect(await body.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
        await body.hover();
        await page.mouse.wheel(0, 400);
        await expect.poll(() => body.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
        expect(await page.evaluate(() => scrollY)).toBe(tall.scrolled);
        const shrink = await record(page, () => press(dialog, "Back"));
        await expectAnchoredThroughout(page, shrink);
        await anchoredAtNaturalSize(page, dialog);
      });

      test("the result arriving while a step change still animates widens the dialog from where it is, without errors", async ({ page }) => {
        const dialog = await openExample(page, "result");
        await dialog.getByRole("textbox", { name: "Name" }).fill("checkout-web");
        const samples = await record(page, () =>
          dialog.evaluate(async (box) => {
            const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
            const button = (name: string) => [...box.querySelectorAll<HTMLElement>("[data-dialog-footer] button")].find((b) => b.innerText.trim() === name);
            button("Continue")?.click();
            for (let i = 0; i < 3; i++) await frame();
            button("Create key")?.click();
          }),
        );
        await expectAnchoredThroughout(page, samples);
        await expect(dialog).toHaveAccessibleName("Key created");
        const after = await anchoredAtNaturalSize(page, dialog);
        if (viewport.width > 760) expect(after.width).toBe(720);
      });

      test("the result of the last step takes the steps' place in the same dialog, wider, from the same top edge", async ({ page }) => {
        const dialog = await openExample(page, "result");
        await dialog.getByRole("textbox", { name: "Name" }).fill("checkout-web");
        await record(page, () => press(dialog, "Continue"));
        const before = await anchoredAtNaturalSize(page, dialog);
        const morph = await record(page, () => press(dialog, "Create key"));
        await expectAnchoredThroughout(page, morph);
        expectSmooth(morph, "h");
        if (viewport.width > 760) expectSmooth(morph, "w");
        await expect(dialog).toHaveAccessibleName("Key created");
        const after = await anchoredAtNaturalSize(page, dialog);
        expect(after.height).toBeGreaterThan(before.height);
        if (viewport.width > 760) expect(after.width).toBe(720);
        await expect(dialog.getByRole("button", { name: "Done" })).toBeFocused();
      });

      test("a dialog whose content grows and shrinks, an error and revealed fields, stays centered at its content's height", async ({ page }) => {
        const dialog = await openExample(page, "changing");
        const first = await centredAtNaturalSize(dialog);
        const error = await record(page, () => press(dialog, "Send invitation"));
        expectCentredThroughout(error);
        await expect(dialog.getByText("Enter a full address, like ana@northwind.example")).toBeVisible();
        const withError = await centredAtNaturalSize(dialog);
        expect(withError.height).toBeGreaterThanOrEqual(first.height);
        const reveal = await record(page, () => dialog.getByRole("checkbox", { name: "Add them to projects now" }).click());
        expectCentredThroughout(reveal);
        expectSmooth(reveal, "h");
        const revealed = await centredAtNaturalSize(dialog);
        expect(revealed.height).toBeGreaterThan(withError.height + 40);
        await dialog.getByRole("textbox", { name: "Email address" }).fill("ana@northwind.example");
        const hide = await record(page, () => dialog.getByRole("checkbox", { name: "Add them to projects now" }).click());
        expectCentredThroughout(hide);
        expectSmooth(hide, "h");
        const last = await centredAtNaturalSize(dialog);
        expect(last.height).toBeLessThan(withError.height + 1);
      });
    });
  }
}

test("the command palette keeps its top while its results shrink and grow smoothly, without a policy violation", async ({ page }) => {
  await open(page, "/components/command-palette/examples");
  const trigger = page.locator("#ex-basic .pv-stage button").first();
  await trigger.scrollIntoViewIfNeeded();
  await hydrated(page);
  await trigger.click();
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  await still(page);
  const top = async () => (await palette.boundingBox())?.y ?? 0;
  const before = await top();
  const tall = (await palette.boundingBox())?.height ?? 0;
  const height = async () => (await palette.boundingBox())?.height ?? 0;
  await palette.getByRole("combobox").pressSequentially("keys");
  await expect.poll(height).toBeLessThan(tall - 40);
  await expect(palette).not.toHaveAttribute("data-resizing");
  expect(Math.abs((await top()) - before)).toBeLessThanOrEqual(1);
  await palette.getByRole("combobox").fill("");
  await expect.poll(async () => Math.abs((await height()) - tall)).toBeLessThanOrEqual(1);
  await expect(palette).not.toHaveAttribute("data-resizing");
  expect(Math.abs((await top()) - before)).toBeLessThanOrEqual(1);
});

test("a step left while its select is still closing comes back with the choice, at the same top edge and without a browser error", async ({ page }) => {
  const dialog = await openExample(page, "steps");
  await dialog.getByRole("textbox", { name: "Name" }).fill("checkout-web");
  await press(dialog, "Continue");
  const expires = dialog.getByRole("combobox", { name: "Expires" });
  for (const choice of ["Never", "In 30 days", "In 90 days", "Never"]) {
    await expires.click();
    await page.getByRole("option", { name: choice }).click();
    await press(dialog, "Back");
    await press(dialog, "Continue");
    await expect(expires).toHaveText(choice);
  }
  await still(page);
  await anchoredAtNaturalSize(page, dialog);
});

test.describe("with reduced motion", () => {
  test.use({ reducedMotion: "reduce" });
  test("a step dialog takes the new step's height at once and keeps its top edge", async ({ page }) => {
    const dialog = await openExample(page, "step-heights");
    const samples = await record(page, () => press(dialog, "Continue"));
    await expectAnchoredThroughout(page, samples);
    expect(new Set(samples.map((s) => Math.round(s.h))).size).toBeLessThanOrEqual(2);
    await anchoredAtNaturalSize(page, dialog);
  });
});

for (const { place, example, filter } of [
  { place: "above", example: "create", filter: "checkout" },
  { place: "below", example: "single", filter: "claude-haiku" },
] as const) {
  test(`an anchored list that shrinks while it filters stays against its field ${place} it`, async ({ page }) => {
    await open(page, "/components/combobox/examples");
    const field = page.locator(`#ex-${example} .pv-stage`).getByRole("combobox");
    await field.scrollIntoViewIfNeeded();
    await hydrated(page);
    const box = (await field.boundingBox())!;
    if (place === "above") await page.setViewportSize({ width: 1440, height: Math.round(box.y + box.height + 48) });
    await field.click();
    await field.fill("");
    const list = page.getByRole("listbox");
    await expect(list).toBeVisible();
    await still(page);
    const measure = () =>
      list.evaluate((el, example) => {
        const popup = el.closest<HTMLElement>("[data-side]")!;
        const input = document.querySelector<HTMLElement>(`#ex-${example} .pv-stage [role=combobox]`)!.getBoundingClientRect();
        const r = popup.getBoundingClientRect();
        return { side: popup.dataset.side, gap: popup.dataset.side === "top" ? input.top - r.bottom : r.top - input.bottom, height: r.height };
      }, example);
    const before = await measure();
    expect(before.side).toBe(place === "above" ? "top" : "bottom");
    await field.pressSequentially(filter);
    await still(page);
    const after = await measure();
    expect(after.side).toBe(before.side);
    expect(after.height).toBeLessThan(before.height - 20);
    expect(Math.abs(after.gap - before.gap)).toBeLessThanOrEqual(1);
  });
}
