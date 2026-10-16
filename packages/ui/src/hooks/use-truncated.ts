import { useCallback, useLayoutEffect, useState, type RefObject } from "react";

export function useTruncated<T extends HTMLElement>(ref: RefObject<T | null>, enabled = true): boolean {
  const [truncated, setTruncated] = useState(false);
  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setTruncated(el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1);
  }, [ref]);
  useLayoutEffect(() => {
    if (!enabled) return;
    measure();
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [enabled, measure, ref]);
  return truncated;
}
