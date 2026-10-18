import { useLayoutEffect, useState, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

function layoutWidth(el: HTMLElement): number | null {
  if (!el.getClientRects().length) return null;
  const cs = getComputedStyle(el);
  const w = parseFloat(cs.width);
  return cs.boxSizing === "border-box" && Number.isFinite(w) ? w : el.offsetWidth;
}

export function useLayoutWidth(ref: RefObject<HTMLElement | null>, enabled = true): number | null {
  const [width, setWidth] = useState<number | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!enabled || !el) return;
    const measure = () => {
      const next = layoutWidth(el);
      if (next !== null) setWidth(next);
    };
    measure();
    return observeResize([el], measure);
  }, [ref, enabled]);
  return width;
}

export function anchorWidthStyle(width: number | null): Record<string, string> | undefined {
  return width === null ? undefined : { "--k-anchor-width": `${width}px` };
}
