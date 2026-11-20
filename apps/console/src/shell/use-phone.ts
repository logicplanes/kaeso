import { useSyncExternalStore } from "react";

const PHONE = "(max-width: 639px)";
const BELOW_DESKTOP = "(max-width: 1279px)";

function list(query: string): MediaQueryList | null {
  return typeof window.matchMedia === "function" ? window.matchMedia(query) : null;
}

const subscribers = new Map<string, (onChange: () => void) => () => void>();

function subscriber(query: string): (onChange: () => void) => () => void {
  let subscribe = subscribers.get(query);
  if (!subscribe) {
    subscribe = (onChange) => {
      const media = list(query);
      media?.addEventListener("change", onChange);
      return () => media?.removeEventListener("change", onChange);
    };
    subscribers.set(query, subscribe);
  }
  return subscribe;
}

export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(subscriber(query), () => list(query)?.matches ?? false);
}

export function usePhone(): boolean {
  return useMediaQuery(PHONE);
}

export function useBelowDesktop(): boolean {
  return useMediaQuery(BELOW_DESKTOP);
}
