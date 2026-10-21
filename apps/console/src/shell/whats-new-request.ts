import { useSyncExternalStore } from "react";

let asked = 0;
const listeners = new Set<() => void>();

export function showWhatsNew(): void {
  asked += 1;
  for (const l of listeners) l();
}

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export function useWhatsNewAsked(): number {
  return useSyncExternalStore(subscribe, () => asked);
}
