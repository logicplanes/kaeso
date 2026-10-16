import { useLayoutEffect, useState, type RefObject } from "react";

function layoutWidth(el: HTMLElement): number {
  const cs = getComputedStyle(el);
  const w = parseFloat(cs.width);
  return cs.boxSizing === "border-box" && Number.isFinite(w) ? w : el.offsetWidth;
}

export function useLayoutWidth(ref: RefObject<HTMLElement | null>, enabled = true): number | null {
  const [width, setWidth] = useState<number | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!enabled || !el) return;
    setWidth(layoutWidth(el));
    if (typeof ResizeObserver === "undefined") return;
    let frame = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => setWidth(layoutWidth(el)));
    });
    ro.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, [ref, enabled]);
  return width;
}

export function anchorWidthStyle(width: number | null): Record<string, string> | undefined {
  return width === null ? undefined : { "--k-anchor-width": `${width}px` };
}
