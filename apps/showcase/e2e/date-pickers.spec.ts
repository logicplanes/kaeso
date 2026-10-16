import { expect, test, type Page } from "@playwright/test";
import { open } from "./support";

const pickers = [
  { name: "Expires", chunk: /\/date-popup\.[\w-]+\.js$/, dialog: "Choose a date" },
  { name: "Period", chunk: /\/range-popup\.[\w-]+\.js$/, dialog: "Choose a date range" },
] as const;

async function slowMachine(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 6 });
}

function boxesAfterPress(page: Page, dialog: string) {
  return page.evaluate(
    (label) =>
      new Promise<{ pressedFor: number; boxes: string[] }>((done) => {
        let pressed: number | undefined;
        let shown: number | undefined;
        let frame = 0;
        const boxes: string[] = [];
        document.addEventListener("pointerdown", () => (pressed ??= frame), { capture: true, once: true });
        const tick = () => {
          frame++;
          const popup = document.querySelector<HTMLElement>(`[role="dialog"][aria-label="${label}"]`);
          const positioner = popup?.closest('[role="presentation"][data-side]');
          if (popup && positioner) {
            shown ??= frame;
            const at = positioner.getBoundingClientRect();
            boxes.push(`${Math.round(at.x)},${Math.round(at.y)} ${Math.round(at.width)}x${Math.round(at.height)}`);
          }
          if (shown === undefined || frame - shown < 12) requestAnimationFrame(tick);
          else done({ pressedFor: shown - (pressed ?? shown), boxes });
        };
        requestAnimationFrame(tick);
      }),
    dialog,
  );
}

for (const picker of pickers) {
  test(`the date picker "${picker.name}" on a slow machine loads while the page is idle and opens on the press`, async ({ page }) => {
    await slowMachine(page);
    const preloaded = page.waitForResponse(picker.chunk, { timeout: 15_000 });
    await open(page, "/components/date-picker/examples");
    await preloaded;
    const button = page.getByRole("button", { name: picker.name }).first();
    await button.scrollIntoViewIfNeeded();
    const recording = boxesAfterPress(page, picker.dialog);
    await button.click();
    const { pressedFor, boxes } = await recording;
    expect(pressedFor, "frames between the press and the popover").toBeLessThanOrEqual(3);
    expect(boxes.filter((b) => b !== boxes[0])).toEqual([]);
  });

  test(`the date picker "${picker.name}" pressed before it has loaded shows pressed, then opens whole`, async ({ page }) => {
    await slowMachine(page);
    let release!: () => void;
    const arrived = new Promise<void>((resolve) => (release = resolve));
    await page.route(picker.chunk, async (route) => {
      await arrived;
      await route.continue();
    });
    await page.goto("/components/date-picker/examples");
    const button = page.getByRole("button", { name: picker.name }).first();
    await button.scrollIntoViewIfNeeded();
    await expect(page.locator("astro-island[ssr]", { has: button })).toHaveCount(0);
    await button.click();
    await expect(button).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("dialog", { name: picker.dialog })).toHaveCount(0);
    const recording = boxesAfterPress(page, picker.dialog);
    release();
    const { boxes } = await recording;
    expect(boxes.length).toBeGreaterThan(3);
    expect(boxes.filter((b) => b !== boxes[0])).toEqual([]);
  });
}
