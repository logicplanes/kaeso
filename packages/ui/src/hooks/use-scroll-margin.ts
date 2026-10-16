import { useLayoutEffect, useRef, useState, type RefObject } from "react";

export function useScrollMargin(ref: RefObject<HTMLElement | null>, getScrollElement: () => HTMLElement | null, count: number) {
  const [margin, setMargin] = useState(0);
  const scrollElement = useRef(getScrollElement);
  scrollElement.current = getScrollElement;
  useLayoutEffect(() => {
    const el = ref.current;
    const scroller = scrollElement.current();
    if (!el || !scroller) return;
    const page = scroller === document.scrollingElement;
    const measure = () => {
      const origin = page ? 0 : scroller.getBoundingClientRect().top + scroller.clientTop;
      setMargin(Math.round(el.getBoundingClientRect().top - origin + scroller.scrollTop));
    };
    measure();
    const target = page ? window : scroller;
    target.addEventListener("scroll", measure, { passive: true });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(page ? document.body : scroller);
    return () => {
      target.removeEventListener("scroll", measure);
      observer?.disconnect();
    };
  }, [ref, count]);
  return margin;
}
