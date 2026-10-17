import { useSyncExternalStore } from "react";

let open = false;
let query = "";
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

export function setSearchQuery(next: string): void {
  if (query === next) return;
  query = next;
  notify();
}

export function setSearchOpen(next: boolean): void {
  if (open === next) return;
  open = next;
  if (next) query = "";
  notify();
}

export const openSearch = () => setSearchOpen(true);

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSearchOpen(): boolean {
  return useSyncExternalStore(subscribe, () => open);
}

export function useSearchQuery(): string {
  return useSyncExternalStore(subscribe, () => query);
}

const typesIntoSearch = (target: EventTarget | null) => target instanceof Element && target.closest("[data-palette-input]") !== null;

export function listenForSearchKey(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      setSearchOpen(!open);
    } else if (open && !e.metaKey && !e.ctrlKey && !e.altKey && !typesIntoSearch(e.target)) {
      if (e.key.length === 1) {
        e.preventDefault();
        setSearchQuery(query + e.key);
      } else if (e.key === "Backspace") {
        e.preventDefault();
        setSearchQuery(query.slice(0, -1));
      } else if (e.key === "Escape") setSearchOpen(false);
    }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
