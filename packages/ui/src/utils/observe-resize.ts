import { flushSync } from "react-dom";

const due = new Set<() => void>();
let frame: number | null = null;

function commitDue() {
  frame = null;
  const runs = [...due];
  due.clear();
  flushSync(() => {
    for (const run of runs) run();
  });
}

export function observeResize(targets: Iterable<Element>, onResize: (entries: readonly ResizeObserverEntry[]) => void): () => void {
  if (typeof ResizeObserver === "undefined") return () => {};
  let latest: readonly ResizeObserverEntry[] = [];
  const run = () => onResize(latest);
  const observer = new ResizeObserver((entries) => {
    latest = entries;
    due.add(run);
    frame ??= requestAnimationFrame(commitDue);
  });
  for (const target of targets) observer.observe(target);
  return () => {
    due.delete(run);
    observer.disconnect();
  };
}
