import { useSyncExternalStore } from "react";

let instant = false;
const listeners = new Set<() => void>();

export function showTooltipsAtOnce(on: boolean): void {
  instant = on;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useTooltipsAtOnce(): boolean {
  return useSyncExternalStore(subscribe, () => instant);
}
