import { useLayoutEffect, useState, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

const SCROLLS = /(auto|scroll|hidden|overlay)/;

function scrollContainer(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const s = getComputedStyle(p);
    if (SCROLLS.test(s.overflowY) || SCROLLS.test(s.overflowX)) return p;
  }
  return null;
}

// A sticky element stops at its scroll container's content edge, so `top: 0` under a padded container leaves a strip that rows scroll through.
export function useStickyTop(ref: RefObject<HTMLElement | null>): number {
  const [top, setTop] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    const scroller = el && scrollContainer(el);
    if (!scroller) return;
    const measure = () => setTop(-(Number.parseFloat(getComputedStyle(scroller).paddingTop) || 0));
    measure();
    return observeResize([scroller], measure);
  }, [ref]);
  return top;
}
