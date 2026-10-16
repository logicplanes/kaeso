import { expect, test, type Page } from "@playwright/test";
import { open, slowMachine } from "./support";

interface Frame {
  box: string;
  header: boolean;
  moving: boolean;
}

const cases = [
  { path: "/components/menu", trigger: "Key actions", header: false },
  { path: "/components/menu/examples", trigger: "Ada Okafor", header: true },
  { path: "/components/sidebar", trigger: /: switch project$/, header: false },
  { path: "/components/sidebar", trigger: /: switch or open settings$/, header: true },
] as const;

test.use({ reducedMotion: "no-preference" });

for (const menu of cases) {
  test(`${menu.path} "${String(menu.trigger)}" opens in place`, async ({ page }) => {
    await open(page, menu.path);
    const trigger = page.getByRole("button", { name: menu.trigger, exact: typeof menu.trigger === "string" }).first();
    await trigger.scrollIntoViewIfNeeded();
    const recording = page.evaluate(
      () =>
        new Promise<Frame[]>((done) => {
          const frames: Frame[] = [];
          const start = performance.now();
          const tick = () => {
            const list = document.querySelector<HTMLElement>('[role="menu"]');
            const positioner = list?.closest('[role="presentation"][data-side]');
            const popup = positioner && [...positioner.children].find((el): el is HTMLElement => el instanceof HTMLElement && el.contains(list));
            if (list && positioner && popup) {
              const at = positioner.getBoundingClientRect();
              const cs = getComputedStyle(popup);
              frames.push({
                box: `${Math.round(at.x)},${Math.round(at.y)} ${Math.round(at.width)}x${Math.round(at.height)}, popup ${popup.offsetLeft},${popup.offsetTop} ${popup.offsetWidth}x${popup.offsetHeight}, list ${list.offsetWidth}x${list.offsetHeight}, ${list.querySelectorAll('[role^="menuitem"]').length} items`,
                header: popup !== list,
                moving: Number(cs.opacity) < 1 || cs.transform !== "none",
              });
            }
            if (performance.now() - start < 700) requestAnimationFrame(tick);
            else done(frames);
          };
          requestAnimationFrame(tick);
        }),
    );
    await trigger.click();
    const frames = await recording;
    expect(frames.length, "frames with the menu shown").toBeGreaterThanOrEqual(4);
    const [first] = frames;
    const last = frames.at(-1)?.box;
    expect(first?.header, "a header beside the items").toBe(menu.header);
    expect(first?.moving, "open motion in the first frame").toBe(true);
    expect(frames.filter((f) => f.box !== last).map((f) => f.box)).toEqual([]);
  });
}

const SIDEBAR_MENU = /\/sidebar-menu\.[\w-]+\.js$/;

function boxesAfterPress(page: Page) {
  return page.evaluate(
    () =>
      new Promise<{ pressedFor: number; boxes: string[] }>((done) => {
        let pressed: number | undefined;
        let shown: number | undefined;
        let frame = 0;
        const boxes: string[] = [];
        document.addEventListener("pointerdown", () => (pressed ??= frame), { capture: true, once: true });
        const tick = () => {
          frame++;
          const list = document.querySelector<HTMLElement>('[role="menu"]');
          const positioner = list?.closest('[role="presentation"][data-side]');
          if (list && positioner) {
            shown ??= frame;
            const at = positioner.getBoundingClientRect();
            boxes.push(`${Math.round(at.x)},${Math.round(at.y)} ${Math.round(at.width)}x${Math.round(at.height)}, ${list.querySelectorAll('[role^="menuitem"]').length} items`);
          }
          if (shown === undefined || frame - shown < 12) requestAnimationFrame(tick);
          else done({ pressedFor: shown - (pressed ?? shown), boxes });
        };
        requestAnimationFrame(tick);
      }),
  );
}

for (const trigger of [/: switch project$/, /: switch or open settings$/]) {
  test(`the sidebar menu "${String(trigger)}" on a slow machine loads while the page is idle and opens on the press`, async ({ page }) => {
    await slowMachine(page);
    const preloaded = page.waitForResponse(SIDEBAR_MENU, { timeout: 15_000 });
    await open(page, "/components/sidebar");
    await preloaded;
    const button = page.getByRole("button", { name: trigger }).first();
    await button.scrollIntoViewIfNeeded();
    const recording = boxesAfterPress(page);
    await button.click();
    const { pressedFor, boxes } = await recording;
    expect(pressedFor, "frames between the press and the menu").toBeLessThanOrEqual(3);
    expect(boxes.filter((b) => b !== boxes[0])).toEqual([]);
  });

  test(`the sidebar menu "${String(trigger)}" pressed before it has loaded shows pressed, then opens whole`, async ({ page }) => {
    await slowMachine(page);
    let release!: () => void;
    const arrived = new Promise<void>((resolve) => (release = resolve));
    await page.route(SIDEBAR_MENU, async (route) => {
      await arrived;
      await route.continue();
    });
    await page.goto("/components/sidebar");
    const button = page.getByRole("button", { name: trigger }).first();
    await button.scrollIntoViewIfNeeded();
    await expect(page.locator("astro-island[ssr]", { has: button })).toHaveCount(0);
    await button.click();
    await expect(button).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("menu")).toHaveCount(0);
    const recording = boxesAfterPress(page);
    release();
    const { boxes } = await recording;
    expect(boxes.length).toBeGreaterThan(3);
    expect(boxes.filter((b) => b !== boxes[0])).toEqual([]);
  });
}
