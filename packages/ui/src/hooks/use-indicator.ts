import { useEffect, useLayoutEffect, useState, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

export function useIndicator(
  container: RefObject<HTMLElement | null>,
  selector: string,
  layout: string,
): { x: number; y: number; w: number; h: number; ready: boolean; animate: boolean } {
  const [box, setBox] = useState({ x: 0, y: 0, w: 0, h: 0, ready: false });
  const [animate, setAnimate] = useState(false);
  useEffect(() => {
    if (!box.ready || animate) return;
    let raf = 0;
    let live = true;
    void (document.fonts?.ready ?? Promise.resolve()).then(() => {
      if (live) raf = requestAnimationFrame(() => (raf = requestAnimationFrame(() => setAnimate(true))));
    });
    return () => {
      live = false;
      cancelAnimationFrame(raf);
    };
  }, [box.ready, animate]);
  useLayoutEffect(() => {
    const root = container.current;
    if (!root) return;
    const measure = () => {
      const el = root.querySelector<HTMLElement>(selector);
      if (!el) return setBox((b) => ({ ...b, ready: false }));
      setBox({ x: Math.round(el.offsetLeft), y: Math.round(el.offsetTop), w: Math.round(el.offsetWidth), h: Math.round(el.offsetHeight), ready: true });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    let live = true;
    const stop = observeResize([root, ...root.querySelectorAll(":scope > *")], measure);
    void document.fonts?.ready.then(() => {
      if (live) measure();
    });
    return () => {
      live = false;
      stop();
    };
  }, [container, selector, layout]);
  return { ...box, animate };
}
