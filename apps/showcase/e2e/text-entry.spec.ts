import { expect, test, type Page } from "@playwright/test";
import { open } from "./support";

const pane = (page: Page) => page.locator("[data-preview] .pv-stage").first();

test("a selected ButtonGroup item stands out from its group by at least 3:1 in both themes", async ({ page }) => {
  await open(page, "/components/button-group/states");
  const ratios = await page.evaluate(() => {
    const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
    if (!ctx) throw new Error("no 2d canvas");
    const rgb = (colour: string) => {
      ctx.clearRect(0, 0, 1, 1);
      ctx.fillStyle = colour;
      ctx.fillRect(0, 0, 1, 1);
      return [...ctx.getImageData(0, 0, 1, 1).data].slice(0, 3);
    };
    const lum = (c: number[]) => {
      const [r = 0, g = 0, b = 0] = c.map((v) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4));
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    return ["light", "dark"].map((theme) => {
      const cell = document.querySelector(`[data-states-theme="${theme}"] [data-cell="attached-selected"]`);
      const selected = cell?.querySelector("[aria-pressed=true]");
      const group = cell?.querySelector("[role=group]");
      if (!selected || !group) return 0;
      const [a, b] = [lum(rgb(getComputedStyle(selected).backgroundColor)), lum(rgb(getComputedStyle(group).backgroundColor))];
      return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    });
  });
  for (const ratio of ratios) expect(ratio).toBeGreaterThanOrEqual(3);
});

test("OTPInput puts the caret on the clicked cell and fills every cell from a code pasted with a space", async ({ page }) => {
  await open(page, "/components/otp-input");
  const input = pane(page).getByRole("textbox", { name: /code/i });
  await input.focus();
  await page.keyboard.type("418207");
  const third = pane(page).locator(".material-field").nth(2);
  const box = await third.boundingBox();
  if (!box) throw new Error("no cell");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(third).toHaveAttribute("data-focused", "");
  await page.keyboard.type("5");
  await expect(input).toHaveValue("415207");
  await page.keyboard.press("ArrowLeft");
  await expect(pane(page).locator(".material-field").nth(2)).toHaveAttribute("data-focused", "");
  await input.fill("");
  await page.evaluate(() => {
    const el = document.querySelector<HTMLInputElement>("[data-preview] .pv-stage input[autocomplete=one-time-code]");
    const data = new DataTransfer();
    data.setData("text", "552 019");
    el?.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  });
  await expect(input).toHaveValue("552019");
});

test("a horizontal Field's label sits on its control's text line at every size", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "/components/field/states");
  const offsets = await page.evaluate(() => {
    const section = document.querySelector('[data-states-theme="light"]');
    const middle = (el: Element) => {
      const r = el.getBoundingClientRect();
      return r.top + r.height / 2;
    };
    return ["sm", "md", "lg", "xl"].map((size) => {
      const label = [...(section?.querySelectorAll("label") ?? [])].find((l) => l.textContent === `Name (${size})`);
      const input = label && document.getElementById(label.htmlFor);
      return label && input ? Math.abs(middle(label) - middle(input)) : 99;
    });
  });
  for (const offset of offsets) expect(offset).toBeLessThanOrEqual(2);
});

test("a Fieldset is named by its legend", async ({ page }) => {
  await open(page, "/components/field");
  await expect(pane(page).getByRole("group", { name: "New key" })).toBeVisible();
});
