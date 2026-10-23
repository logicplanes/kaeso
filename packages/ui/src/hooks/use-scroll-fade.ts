import { useEffect, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

function setLength(el: HTMLElement, name: string, px: number) {
  if (px > 0) el.style.setProperty(name, `${px}px`);
  else el.style.removeProperty(name);
}

export function useScrollFadeX(ref: RefObject<HTMLElement | null>, content: string, key?: unknown) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => {
      const start = Math.abs(el.scrollLeft);
      setLength(el, "--scroll-area-overflow-x-start", start);
      setLength(el, "--scroll-area-overflow-x-end", el.scrollWidth - el.clientWidth - start);
    };
    update();
    el.addEventListener("scroll", update, { passive: true });
    const inner = el.querySelector(content);
    const stop = observeResize(inner ? [el, inner] : [el], update);
    return () => {
      el.removeEventListener("scroll", update);
      stop();
    };
  }, [ref, content, key]);
}
