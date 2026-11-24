import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { nextFrame } from "../utils/observe-resize.js";
import { isCut } from "./use-truncated.js";

// Text that may be cut off measures itself only from one shared observer, never while React commits. Reading its width in a layout
// effect forced the browser to lay out the page once per commit, and a table has a span per cell: hundreds of reads and observers made
// every page of rows and every resize slow. The observer calls back right after the browser's own layout, where the read is free.
let shared: { observer: ResizeObserver; of: typeof ResizeObserver } | undefined;
const reports = new Map<Element, (cut: boolean) => void>();

function watchCut(el: Element, report: (cut: boolean) => void): () => void {
  if (typeof ResizeObserver === "undefined") {
    report(isCut(el));
    return () => {};
  }
  if (shared?.of !== ResizeObserver) {
    const check = (targets: Iterable<Element>) => {
      for (const target of targets) {
        const to = reports.get(target);
        const cut = to && isCut(target);
        if (to) nextFrame(() => reports.get(target) === to && to(!!cut));
      }
    };
    const observer = new ResizeObserver((entries) => check(entries.map((e) => e.target)));
    // A web font that arrives after the first layout widens the text but not its box, which the observer never reports.
    document.fonts?.addEventListener("loadingdone", () => check([...reports.keys()]));
    shared = { observer, of: ResizeObserver };
  }
  const { observer } = shared;
  reports.set(el, report);
  observer.observe(el);
  return () => {
    if (reports.get(el) === report) reports.delete(el);
    observer.unobserve(el);
  };
}

// Whether text is cut off, for text where that only changes what hovering and focusing it does (a tooltip, a tab stop), never what is
// drawn: the answer comes a frame after the text appears, from the shared observer above. Give the returned ref to the text's element.
export function useCutOff(enabled = true): [cut: boolean, ref: (el: Element | null) => void] {
  const [cut, setCut] = useState(false);
  const watched = useRef<{ el: Element; stop: () => void } | null>(null);
  // The element can change under the same component (Text puts its span inside a tooltip once it is cut), so the ref follows it.
  // A ref is set again on every render that makes a new one, so a null in between is ignored: the watch ends when another element
  // takes its place, when it is turned off, or on unmount.
  const ref = useCallback(
    (el: Element | null) => {
      if (!enabled || !el || watched.current?.el === el) return;
      watched.current?.stop();
      watched.current = { el, stop: watchCut(el, setCut) };
    },
    [enabled],
  );
  useLayoutEffect(
    () => () => {
      watched.current?.stop();
      watched.current = null;
    },
    [enabled],
  );
  return [enabled && cut, ref];
}
