import { lazy, type LazyExoticComponent, type ReactNode } from "react";

type PendingView = () => ReactNode;

export function lazyPending(load: () => Promise<PendingView>): LazyExoticComponent<PendingView> & { preload: () => Promise<void> } {
  const module = () => load().then((component) => ({ default: component }));
  return Object.assign(lazy(module), { preload: () => module().then(() => undefined) });
}
