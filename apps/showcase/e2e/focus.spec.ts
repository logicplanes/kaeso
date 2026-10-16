import { expect, test, type Page } from "@playwright/test";
import { examplePages, growToContent, open } from "./support";

test.use({ viewport: { width: 1280, height: 900 } });

async function focusColor(page: Page) {
  return page.evaluate(() => {
    const probe = document.createElement("span");
    probe.style.color = "var(--k-focus)";
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });
}

test.describe("no focus ring after a click", () => {
  for (const path of examplePages) {
    test(path, async ({ page }) => {
      await open(page, path);
      await growToContent(page, 1280);
      const focus = await focusColor(page);
      const targets = await page
        .locator(
          "[data-preview] .pv-stage :is(button, [role=tab], [role=radio], [role=switch], [role=checkbox]):not([disabled]):not([aria-disabled=true]):not([aria-haspopup]):not([data-force])",
        )
        .elementHandles();
      const found: string[] = [];
      let checked = 0;
      for (const target of targets.slice(0, 24)) {
        const reachable = await target.evaluate((el: Element) => {
          if (!el.isConnected) return false;
          el.scrollIntoView({ block: "nearest", inline: "nearest" });
          const r = el.getBoundingClientRect();
          const hit = r.width && r.height ? document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) : null;
          return !!hit && el.contains(hit);
        });
        if (!reachable) continue;
        await target.click({ timeout: 5000 });
        checked++;
        const ring = await target.evaluate(async (el, color) => {
            await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            const active = document.activeElement;
            if (!el.isConnected || !active || (active !== el && !el.contains(active))) return null;
            const nodes = [active, ...active.querySelectorAll("*")].slice(0, 6);
            const look = () =>
              nodes.map((n) => {
                const cs = getComputedStyle(n);
                return [cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0 && cs.outlineColor === color, cs.boxShadow.includes(color)];
              });
            const focused = look();
            (active as HTMLElement).blur();
            await new Promise((resolve) => requestAnimationFrame(resolve));
            const blurred = look();
            for (const [i, node] of nodes.entries()) {
              if (focused[i]?.[0] && !blurred[i]?.[0]) return `outline on ${node.tagName.toLowerCase()}`;
              if (focused[i]?.[1] && !blurred[i]?.[1]) return `ring shadow on ${node.tagName.toLowerCase()}`;
            }
            return null;
          }, focus);
        if (ring) {
          const name = ((await target.textContent()) ?? "").trim().slice(0, 24) || (await target.getAttribute("aria-label"));
          found.push(`"${name}" keeps a ${ring} after a click`);
        }
        await page.mouse.click(2, 2);
      }
      expect(found).toEqual([]);
      if (targets.length) expect(checked, "no control in the previews was clicked").toBeGreaterThan(0);
    });
  }
});

test.describe("visible focus for the keyboard", () => {
  for (const path of examplePages) {
    test(path, async ({ page }) => {
      await open(page, path);
      await growToContent(page, 1280);
      const started = await page.evaluate(() => {
        const stage = document.querySelector("[data-preview] .pv-stage");
        if (!stage) return false;
        const start = document.createElement("button");
        start.setAttribute("data-audit-start", "");
        stage.prepend(start);
        start.focus();
        return true;
      });
      test.skip(!started, "no preview on this page");
      const seen = new Set<string>();
      const found: string[] = [];
      for (let i = 0; i < 60; i++) {
        await page.keyboard.press("Tab");
        const step = await page.evaluate(async () => {
          await new Promise((resolve) => requestAnimationFrame(resolve));
          const active = document.activeElement as HTMLElement | null;
          if (
            !active ||
            active === document.body ||
            !active.closest("[data-preview] .pv-stage") ||
            active.closest("[role=dialog],[role=menu],[role=listbox]")
          ) {
            return null;
          }
          const nodes = [active, active.parentElement, ...active.querySelectorAll("*")].filter((n): n is Element => !!n).slice(0, 8);
          const look = () =>
            nodes
              .map((n) => {
                const cs = getComputedStyle(n);
                return [cs.outlineColor, cs.outlineStyle, cs.boxShadow, cs.backgroundColor, cs.color, cs.textDecorationColor].join("|");
              })
              .join("~");
          const focused = look();
          active.blur();
          await new Promise((resolve) => requestAnimationFrame(resolve));
          const blurred = look();
          active.focus();
          const role = active.getAttribute("role");
          return {
            key: active.id || active.outerHTML.slice(0, 120),
            same: focused === blurred,
            label: `${active.tagName.toLowerCase()}${role ? `[role=${role}]` : ""} "${(active.getAttribute("aria-label") ?? active.textContent ?? "").trim().slice(0, 28)}"`,
          };
        });
        if (!step) continue;
        if (seen.has(step.key)) break;
        seen.add(step.key);
        if (step.same) found.push(`${step.label} shows no focus`);
      }
      expect(found).toEqual([]);
    });
  }
});
