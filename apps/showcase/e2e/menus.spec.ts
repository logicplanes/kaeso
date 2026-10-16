import { expect, test } from "@playwright/test";
import { open } from "./support";

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
