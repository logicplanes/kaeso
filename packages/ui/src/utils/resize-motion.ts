import { useLayoutEffect, useRef } from "react";

interface Size {
  w: number;
  h: number;
}

const px = (v: string) => parseFloat(v) || 0;
const ms = (v: string) => px(v) * (/\ds\s*$/.test(v) ? 1000 : 1);
const sizeOf = (box: HTMLElement): Size => {
  const s = getComputedStyle(box);
  return { w: px(s.width), h: px(s.height) };
};
const viewport = () => `${window.innerWidth}x${window.innerHeight}`;
const reducedMotion = () => !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
const isBody = (el: Element) => el.hasAttribute("data-dialog-body");

export function resizeSources(box: Element): Element[] {
  const kids = [...box.children];
  const only = kids.length === 1 ? kids[0] : undefined;
  if (only && !isBody(only)) return resizeSources(only);
  return kids.flatMap((c) => (isBody(c) ? [...c.children] : [c]));
}

export function useResizeMotion(box: HTMLElement | null, onSettle?: (() => void) | undefined) {
  const settled = useRef(onSettle);
  const sync = useRef<(() => void) | null>(null);
  useLayoutEffect(() => {
    settled.current = onSettle;
    sync.current?.();
  });
  useLayoutEffect(() => {
    if (!box || typeof ResizeObserver === "undefined") return;
    let size: Size | null = null;
    let view = viewport();
    let motion: Animation | null = null;
    const watched = new Set<Element>();
    const stop = () => {
      motion?.cancel();
      motion = null;
      box.removeAttribute("data-resizing");
      for (const body of box.querySelectorAll("[data-resize-clip]")) body.removeAttribute("data-resize-clip");
    };
    const change = () => {
      const from = motion ? sizeOf(box) : size;
      stop();
      const to = sizeOf(box);
      size = to;
      const resized = view !== (view = viewport());
      const style = getComputedStyle(box);
      const duration = ms(style.getPropertyValue("--k-duration-dialog"));
      const same = !from || (Math.abs(from.h - to.h) < 0.5 && Math.abs(from.w - to.w) < 0.5);
      if (same || resized || !duration || reducedMotion() || typeof box.animate !== "function" || box.matches("[data-ending-style], [data-closed]")) {
        settled.current?.();
        return;
      }
      for (const body of box.querySelectorAll<HTMLElement>("[data-dialog-body]"))
        if (body.scrollHeight <= body.clientHeight) body.setAttribute("data-resize-clip", "");
      box.setAttribute("data-resizing", "");
      const run = box.animate(
        [
          { width: `${from.w}px`, height: `${from.h}px` },
          { width: `${to.w}px`, height: `${to.h}px` },
        ],
        { duration, easing: style.getPropertyValue("--k-ease-standard").trim() || "ease-out" },
      );
      motion = run;
      run.onfinish = () => {
        if (motion !== run) return;
        stop();
        settled.current?.();
      };
    };
    const observer = new ResizeObserver(change);
    sync.current = () => {
      const now = new Set(resizeSources(box));
      let gone = false;
      for (const el of watched) {
        if (now.has(el)) continue;
        observer.unobserve(el);
        watched.delete(el);
        gone = true;
      }
      for (const el of now) {
        if (watched.has(el)) continue;
        observer.observe(el);
        watched.add(el);
      }
      if (gone) change();
    };
    sync.current();
    return () => {
      sync.current = null;
      observer.disconnect();
      stop();
    };
  }, [box]);
}
