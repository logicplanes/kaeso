import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

export function useScrollMargin(ref: RefObject<HTMLElement | null>, getScrollElement: () => HTMLElement | null, count: number, enabled = true) {
  const [margin, setMargin] = useState(0);
  const scrollElement = useRef(getScrollElement);
  scrollElement.current = getScrollElement;
  useLayoutEffect(() => {
    const el = ref.current;
    const scroller = scrollElement.current();
    if (!enabled || !el || !scroller) return;
    const page = scroller === document.scrollingElement;
    const measure = () => {
      const origin = page ? 0 : scroller.getBoundingClientRect().top + scroller.clientTop;
      setMargin(Math.round(el.getBoundingClientRect().top - origin + scroller.scrollTop));
    };
    measure();
    const target = page ? window : scroller;
    target.addEventListener("scroll", measure, { passive: true });
    const stop = observeResize([page ? document.body : scroller], measure);
    return () => {
      target.removeEventListener("scroll", measure);
      stop();
    };
  }, [ref, count, enabled]);
  return margin;
}
