import { useCallback, useLayoutEffect, useState, type RefObject } from "react";
import { observeResize } from "../utils/observe-resize.js";

export const isCut = (el: Element) => el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1;

export function useTruncated<T extends HTMLElement>(ref: RefObject<T | null>, enabled = true): boolean {
  const [truncated, setTruncated] = useState(false);
  const measure = useCallback(() => {
    const el = ref.current;
    if (el) setTruncated(isCut(el));
  }, [ref]);
  useLayoutEffect(() => {
    if (!enabled) return;
    measure();
    const el = ref.current;
    if (!el) return;
    return observeResize([el], measure);
  }, [enabled, measure, ref]);
  return truncated;
}
