import { expect, test } from "@playwright/test";
import { open, slowMachine } from "./support";

test.use({ reducedMotion: "no-preference" });

test("an opening collapsible never scrolls its content inside the clip, so a press lands where it was aimed", async ({ page }) => {
  await slowMachine(page);
  await open(page, "/components/collapsible/examples");
  const trigger = page.getByRole("button", { name: "Advanced" }).first();
  await trigger.scrollIntoViewIfNeeded();
  const frames = await trigger.evaluate(
    (button) =>
      new Promise<{ moving: boolean; offset: number; scrollTop: number }[]>((done) => {
        const seen: { moving: boolean; offset: number; scrollTop: number }[] = [];
        (button as HTMLElement).click();
        const tick = () => {
          const panel = document.getElementById(button.getAttribute("aria-controls") ?? "");
          const content = panel?.firstElementChild;
          if (panel && content) {
            content.lastElementChild?.scrollIntoView({ block: "end" });
            const moving = panel.getAnimations().length > 0 || panel.hasAttribute("data-starting-style");
            seen.push({ moving, offset: Math.round(content.getBoundingClientRect().top - panel.getBoundingClientRect().top), scrollTop: panel.scrollTop });
            if (!moving && seen.length > 1) return done(seen);
          }
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }),
  );
  expect(frames.filter((f) => f.moving).length, "frames sampled while the panel opens").toBeGreaterThan(2);
  expect(frames.filter((f) => f.offset !== 0 || f.scrollTop !== 0)).toEqual([]);
  const heights = await trigger.evaluate((button) => {
    const panel = document.getElementById(button.getAttribute("aria-controls") ?? "");
    return { panel: panel?.getBoundingClientRect().height, content: panel?.firstElementChild?.getBoundingClientRect().height };
  });
  expect(heights.panel).toBeGreaterThan(0);
  expect(heights.panel).toBe(heights.content);
});

test("a list checkbox pressed before its code has loaded selects the row, and the checkbox that arrives keeps it", async ({ page }) => {
  await slowMachine(page);
  let release!: () => void;
  const arrived = new Promise<void>((resolve) => (release = resolve));
  let held = 0;
  await page.route(/\/checkbox\.[\w-]+\.js$/, async (route) => {
    held++;
    await arrived;
    await route.continue();
  });
  await page.goto("/components/list/examples");
  const list = page.getByRole("grid", { name: "Keys" });
  const cell = list
    .getByRole("gridcell")
    .filter({ has: page.getByRole("checkbox", { name: /^Select / }) })
    .first();
  const box = cell.getByRole("checkbox");
  await box.scrollIntoViewIfNeeded();
  await expect(page.locator("astro-island[ssr]", { has: box })).toHaveCount(0);
  await expect.poll(() => held, { message: "the checkbox's code is still on its way" }).toBeGreaterThan(0);
  await expect(box).not.toHaveAttribute("data-unchecked");

  await box.click();
  await expect(box).toHaveAttribute("aria-checked", "true");
  await expect(cell).toHaveAttribute("aria-selected", "true");

  release();
  await page.mouse.move(0, 0);
  await expect(box).toHaveAttribute("data-checked");
  await expect(box).toHaveAttribute("aria-checked", "true");
  await expect(cell).toHaveAttribute("aria-selected", "true");

  await box.click();
  await expect(box).toHaveAttribute("aria-checked", "false");
  await expect(cell).toHaveAttribute("aria-selected", "false");
});
