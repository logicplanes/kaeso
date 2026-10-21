import { useSyncExternalStore } from "react";

const taken = new Map<string, string>();

function entryKey(): string {
  const state: unknown = window.history.state;
  return state && typeof state === "object" && "__TSR_key" in state ? String(state.__TSR_key) : "";
}

export function takeFragment(name: string): string | null {
  const params = new URLSearchParams(window.location.hash.slice(1));
  const value = params.get(name);
  const key = `${entryKey()}${window.location.pathname}#${name}`;
  if (value !== null) {
    taken.set(key, value);
    window.history.replaceState(window.history.state, "", window.location.pathname + window.location.search);
  }
  return taken.get(key) ?? null;
}

const LINK_FRAGMENT = /(?:^|&)(?:token|sso_error)=/;
let linkVisits = 0;
const visitListeners = new Set<() => void>();

function onHashChange() {
  if (!LINK_FRAGMENT.test(window.location.hash.slice(1))) return;
  linkVisits++;
  for (const listener of visitListeners) listener();
}

function subscribeLinkVisits(listener: () => void): () => void {
  if (visitListeners.size === 0) window.addEventListener("hashchange", onHashChange);
  visitListeners.add(listener);
  return () => {
    visitListeners.delete(listener);
    if (visitListeners.size === 0) window.removeEventListener("hashchange", onHashChange);
  };
}

export const useLinkVisit = (): number => useSyncExternalStore(subscribeLinkVisits, () => linkVisits);
