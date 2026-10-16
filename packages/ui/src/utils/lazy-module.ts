import { useEffect, useReducer } from "react";

export interface LazyModule<T> {
  current: () => T | undefined;
  load: () => Promise<T>;
}

let loading: Set<Promise<unknown>> | undefined;

function track<T>(promise: Promise<T>): Promise<T> {
  const all = (loading ??= new Set());
  all.add(promise);
  const settle = () => all.delete(promise);
  promise.then(settle, settle);
  return promise;
}

export async function lazyModulesSettled(): Promise<void> {
  while (loading?.size) await Promise.allSettled(loading);
}

export function lazyModule<T>(load: () => Promise<T>): LazyModule<T> {
  let mod: T | undefined;
  let pending: Promise<T> | undefined;
  return {
    current: () => mod,
    load: () =>
      (pending ??= track(
        load().then(
          (m) => (mod = m),
          (error: unknown) => {
            pending = undefined;
            throw error;
          },
        ),
      )),
  };
}

export function preload<T>(m: LazyModule<T>): void {
  m.load().catch(() => undefined);
}

export function useLazyModule<T>(m: LazyModule<T>, wanted: boolean): T | undefined {
  const [, loaded] = useReducer((n: number) => n + 1, 0);
  const mod = m.current();
  useEffect(() => {
    if (!wanted || mod) return;
    let live = true;
    m.load().then(
      () => live && loaded(),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [m, wanted, mod]);
  return wanted ? mod : undefined;
}
