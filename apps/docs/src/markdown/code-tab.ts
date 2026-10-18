import { useSyncExternalStore } from "react";

export const CODE_TAB_STORAGE_KEY = "kaeso-docs:code-tab";

const listeners = new Set<() => void>();
let chosen: string | null | undefined;

function read(): string | null {
  try {
    return localStorage.getItem(CODE_TAB_STORAGE_KEY);
  } catch {
    return null;
  }
}

function remember(label: string): void {
  try {
    localStorage.setItem(CODE_TAB_STORAGE_KEY, label);
  } catch {
    return;
  }
}

function snapshot(): string | undefined {
  if (chosen === undefined) chosen = read();
  return chosen ?? undefined;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function chooseCodeTab(label: string): void {
  chosen = label;
  remember(label);
  for (const listener of listeners) listener();
}

export function forgetCodeTab(): void {
  chosen = undefined;
}

export function useCodeTab(): string | undefined {
  return useSyncExternalStore(subscribe, snapshot);
}
