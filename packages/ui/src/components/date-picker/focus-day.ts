import { useCallback } from "react";

export function useFocusDay() {
  return useCallback((root: HTMLDivElement | null) => {
    if (!root) return;
    // Base UI focuses the popup itself when it opens; the grid's day takes focus on the next frame, after that.
    const frame = requestAnimationFrame(() => root.querySelector<HTMLButtonElement>('[role="grid"] button[tabindex="0"]')?.focus({ preventScroll: true }));
    return () => cancelAnimationFrame(frame);
  }, []);
}
