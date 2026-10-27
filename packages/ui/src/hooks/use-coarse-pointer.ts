import { useSyncExternalStore } from "react";

const COARSE = "(pointer: coarse)";

function subscribe(onChange: () => void) {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const query = window.matchMedia(COARSE);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

const coarse = () => typeof window !== "undefined" && !!window.matchMedia?.(COARSE).matches;

export function useCoarsePointer(): boolean {
  return useSyncExternalStore(subscribe, coarse, () => false);
}
