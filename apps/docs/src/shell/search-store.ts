import { useSyncExternalStore } from "react";

let open = false;
let mounted = false;
let typed = "";
const listeners = new Set<() => void>();

export function takeTypedQuery(): string {
  mounted = true;
  const query = typed;
  typed = "";
  return query;
}

export function setSearchOpen(next: boolean): void {
  if (open === next) return;
  open = next;
  if (!next) typed = "";
  for (const listener of listeners) listener();
}

export const openSearch = () => setSearchOpen(true);

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSearchOpen(): boolean {
  return useSyncExternalStore(subscribe, () => open);
}

export function listenForSearchKey(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      setSearchOpen(!open);
    } else if (open && !mounted && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (e.key.length === 1) typed += e.key;
      else if (e.key === "Backspace") typed = typed.slice(0, -1);
      else if (e.key === "Escape") setSearchOpen(false);
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
