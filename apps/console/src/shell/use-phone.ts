import { useSyncExternalStore } from "react";

const PHONE = "(max-width: 639px)";

function query(): MediaQueryList | null {
  return typeof window.matchMedia === "function" ? window.matchMedia(PHONE) : null;
}

function subscribe(onChange: () => void): () => void {
  const list = query();
  list?.addEventListener("change", onChange);
  return () => list?.removeEventListener("change", onChange);
}

export function usePhone(): boolean {
  return useSyncExternalStore(subscribe, () => query()?.matches ?? false);
}
