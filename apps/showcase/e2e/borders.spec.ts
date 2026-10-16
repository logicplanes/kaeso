import { expect, test, type Page } from "@playwright/test";
import { PNG } from "pngjs";
import { open, statesPages } from "./support";

const SCALE = 2;
const PER_PAGE = 40;
const EVEN = 0.6;
const ELEVATED = 0.35;
const GAP = 8;
const FLOOR = 6;

type Side = "top" | "right" | "bottom" | "left";
interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}
interface Candidate {
  id: string;
  label: string;
  radius: number;
  elevated: boolean;
  outset: boolean;
  bg: number | null;
  ringColor: string | null;
}

function luminance(png: PNG, x: number, y: number) {
  const i = (png.width * y + x) * 4;
  return 0.2126 * (png.data[i] ?? 0) + 0.7152 * (png.data[i + 1] ?? 0) + 0.0722 * (png.data[i + 2] ?? 0);
}

function sideContrast(png: PNG, box: Box, side: Side, radius: number, item: Candidate): number | null {
  const { x, y, w, h } = box;
  const r = Math.ceil(radius) + 2;
  const horizontal = side === "top" || side === "bottom";
  const along = horizontal ? [x + r, x + w - 1 - r] : [y + r, y + h - 1 - r];
  let [from = 0, to = 0] = along;
  if (to - from < 4) from = to = horizontal ? x + Math.floor(w / 2) : y + Math.floor(h / 2);
  const ring = Array.from({ length: SCALE }, (_, i) => (item.outset ? -1 - i : i));
  const inward = 2 * SCALE + 1;
  const at = (a: number, d: number): [number, number] => {
    if (side === "top") return [a, y + d];
    if (side === "bottom") return [a, y + h - 1 - d];
    if (side === "left") return [x + d, a];
    return [x + w - 1 - d, a];
  };
  const inside = (px: number, py: number) => px >= 0 && py >= 0 && px < png.width && py < png.height;
  const values: number[] = [];
  const step = Math.max(1, Math.floor((to - from) / 12));
  for (let a = from; a <= to; a += step) {
    const background = item.bg ?? luminance(png, ...at(a, inward));
    const [ox, oy] = at(a, item.outset ? -2 * SCALE - 2 : -3);
    if (!inside(ox, oy)) return null;
    const outside = luminance(png, ox, oy);
    let best = 0;
    for (const shift of item.outset ? [-1, 0] : [0, 1]) {
      let toInside = 0;
      let toOutside = 0;
      for (const d of ring) {
        const [px, py] = at(a, d + shift);
        if (!inside(px, py)) return null;
        const v = luminance(png, px, py);
        toInside += Math.abs(v - background);
        toOutside += Math.abs(v - outside);
      }
      best = Math.max(best, (item.elevated ? toInside : Math.min(toInside, toOutside)) / ring.length);
    }
    values.push(best);
  }
  values.sort((p, q) => p - q);
  return values[Math.floor(values.length / 2)] ?? null;
}

async function candidates(page: Page) {
  return page.evaluate((limit) => {
    const seen = new Set<string>();
    const items: Candidate[] = [];
    const truncated: string[] = [];
    const label = (el: Element) =>
      `${el.tagName.toLowerCase()}${el.getAttribute("aria-label") ? `[${el.getAttribute("aria-label")}]` : ""} "${(el.textContent ?? "").trim().slice(0, 24)}"`;
    const visibleAlpha = (color: string) => {
      const m = /rgba?\(([^)]+)\)/.exec(color);
      if (!m?.[1]) return true;
      const parts = m[1].split(/[ ,/]+/).filter(Boolean);
      return parts.length < 4 || parseFloat(parts[3] ?? "1") > 0.02;
    };
    const opaqueLuminance = (color: string) => {
      const m = /^rgb\((\d+), (\d+), (\d+)\)$/.exec(color);
      return m ? 0.2126 * Number(m[1]) + 0.7152 * Number(m[2]) + 0.0722 * Number(m[3]) : null;
    };
    let n = 0;
    for (const el of document.body.querySelectorAll("*")) {
      if (el.closest("[data-state-label], [aria-hidden=true], .prose-k, pre")) continue;
      const cs = getComputedStyle(el);
      const radius = parseFloat(cs.borderTopLeftRadius);
      if (!(radius > 0) || cs.visibility !== "visible" || parseFloat(cs.opacity) < 1) continue;
      const shadows = cs.boxShadow === "none" ? [] : cs.boxShadow.split(/,(?![^(]*\))/).map((s) => s.trim());
      const ring = shadows.find((s) => /0px 0px 0px 1px/.test(s) && visibleAlpha(s));
      const border = cs.borderTopWidth === "1px" && cs.borderTopStyle !== "none" && visibleAlpha(cs.borderTopColor);
      if (!ring && !border) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 14 || r.height < 14 || r.bottom < 0 || r.top > document.documentElement.scrollHeight) continue;
      if (cs.transform !== "none" && !/^matrix\(1, 0, 0, 1, 0, 0\)$/.test(cs.transform)) continue;
      const theme = el.closest("[data-theme]")?.getAttribute("data-theme");
      const signature = `${theme ?? ""}|${el.tagName}|${String(el.className)}|${Math.round(r.width)}x${Math.round(r.height)}`;
      if (seen.has(signature)) continue;
      seen.add(signature);
      const id = `k-border-${n++}`;
      el.setAttribute("data-k-border", id);
      items.push({
        id,
        label: `${label(el)}${theme ? ` [${theme}]` : ""}`,
        radius,
        elevated: shadows.some((shadow) => {
          const blur = /(-?[\d.]+)px (-?[\d.]+)px ([\d.]+)px/.exec(shadow);
          const alpha = /rgba\([^)]*, ([\d.]+)\)/.exec(shadow);
          return !!blur && parseFloat(blur[3] ?? "0") >= 2 && (!alpha || parseFloat(alpha[1] ?? "1") >= 0.1);
        }),
        outset: !!ring && !/\binset\b/.test(ring) && !border,
        bg: opaqueLuminance(cs.backgroundColor),
        ringColor: ring ? (/rgba?\([^)]*\)/.exec(ring)?.[0] ?? null) : cs.borderTopColor,
      });
      if (items.length >= limit) break;
    }
    for (const el of document.querySelectorAll<HTMLElement>(".rounded-pill .truncate, .rounded-pill.truncate")) {
      const text = (el.textContent ?? "").trim();
      if (!text || text.length > 12 || el.offsetParent === null) continue;
      if (el.scrollWidth > el.clientWidth + 1) truncated.push(label(el.closest(".rounded-pill") ?? el));
    }
    return { items, truncated };
  }, PER_PAGE);
}

async function unobstructed(page: Page, id: string) {
  return page.locator(`[data-k-border="${id}"]`).evaluate((el) => {
    const r = el.getBoundingClientRect();
    const within = (x: number, y: number) => x >= 0 && y >= 0 && x < innerWidth && y < innerHeight;
    const edges: [number, number][] = [
      [r.left + r.width / 2, r.top + 2],
      [r.left + r.width / 2, r.bottom - 2],
      [r.left + 2, r.top + r.height / 2],
      [r.right - 2, r.top + r.height / 2],
    ];
    for (const [x, y] of edges) {
      const hit = within(x, y) ? document.elementFromPoint(x, y) : null;
      if (!hit || !(hit === el || el.contains(hit))) return false;
    }
    if (el.closest("[aria-hidden=true], [data-base-ui-inert]")) return false;
    const modal = document.querySelector("[role=dialog][data-open], [role=alertdialog][data-open]");
    if (modal && !modal.contains(el)) return false;
    const around: [number, number][] = [
      [r.left + r.width / 2, r.top - 3],
      [r.left + r.width / 2, r.bottom + 3],
      [r.left - 3, r.top + r.height / 2],
      [r.right + 3, r.top + r.height / 2],
    ];
    for (const [x, y] of around) {
      if (!within(x, y)) continue;
      const hit = document.elementFromPoint(x, y);
      if (hit && !(hit.contains(el) || el.parentElement?.contains(hit))) return false;
    }
    for (let node: Element | null = el; node; node = node.parentElement) {
      const s = getComputedStyle(node);
      if (s.maskImage && s.maskImage !== "none") return false;
      if (s.filter !== "none" && node !== el) return false;
      if (node !== el && s.transform !== "none" && !/^matrix\(1, 0, 0, 1, [-\d.]+, [-\d.]+\)$/.test(s.transform)) return false;
    }
    return true;
  });
}

test.use({ deviceScaleFactor: SCALE });

test.describe("even edges", () => {
  for (const path of statesPages) {
    test(path, async ({ page }) => {
      await open(page, path);
      const { items, truncated } = await candidates(page);
      const found = truncated.map((t) => `short label truncated: ${t}`);
      for (const item of items) {
        const element = page.locator(`[data-k-border="${item.id}"]`);
        await element.evaluate((el) => el.scrollIntoView({ block: "center", inline: "center" }));
        const box = await element.boundingBox();
        if (!box || !(await unobstructed(page, item.id))) continue;
        const margin = 4;
        const cx = Math.max(0, Math.floor(box.x) - margin);
        const cy = Math.max(0, Math.floor(box.y) - margin);
        const clip = { x: cx, y: cy, width: Math.ceil(box.x + box.width) + margin - cx, height: Math.ceil(box.y + box.height) + margin - cy };
        const png = PNG.sync.read(await page.screenshot({ clip, animations: "disabled", caret: "hide" }));
        const left = Math.round(box.x - cx) * SCALE;
        const top = Math.round(box.y - cy) * SCALE;
        const device = { x: left, y: top, w: Math.round(box.x + box.width - cx) * SCALE - left, h: Math.round(box.y + box.height - cy) * SCALE - top };
        const radius = Math.min(item.radius, box.height / 2, box.width / 2) * SCALE;
        const sides = (["top", "right", "bottom", "left"] as const).map((side) => [side, sideContrast(png, device, side, radius, item)] as const);
        const values = sides.map(([, v]) => v).filter((v): v is number => v !== null);
        if (values.length < 2) continue;
        const max = Math.max(...values);
        const min = Math.min(...values);
        if (max < FLOOR) {
          const m = /rgba?\((\d+), (\d+), (\d+)(?:, ([\d.]+))?\)/.exec(item.ringColor ?? "");
          const reference = item.bg ?? luminance(png, 1, 1);
          const expected = m
            ? (m[4] === undefined ? 1 : parseFloat(m[4])) * Math.abs(0.2126 * Number(m[1]) + 0.7152 * Number(m[2]) + 0.0722 * Number(m[3]) - reference)
            : 0;
          if (expected >= 2 * FLOOR) found.push(`ring not drawn on any side (expected ${expected.toFixed(0)}): ${item.label}`);
          continue;
        }
        if (min < max * (item.elevated ? ELEVATED : EVEN) && max - min > GAP) {
          found.push(`uneven edge (${sides.map(([s, v]) => `${s} ${v === null ? "–" : v.toFixed(0)}`).join(", ")}): ${item.label}`);
        }
      }
      expect(found).toEqual([]);
    });
  }
});
