import { useEffect, useLayoutEffect, useReducer, useRef } from "react";

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

const FAILED_FETCH = /dynamically imported module: (\S+)$/;

function freshUrl(error: unknown, attempt: number): string | undefined {
  const url = error instanceof Error ? FAILED_FETCH.exec(error.message)?.[1] : undefined;
  if (!url || !URL.canParse(url)) return undefined;
  const fresh = new URL(url);
  fresh.searchParams.set("retry", String(attempt));
  return fresh.href;
}

export function lazyModule<T>(load: () => Promise<T>, reimport: (url: string) => Promise<T> = (url) => import(url) as Promise<T>): LazyModule<T> {
  let mod: T | undefined;
  let pending: Promise<T> | undefined;
  let failures = 0;
  let again: string | undefined;
  return {
    current: () => mod,
    load: () =>
      (pending ??= track(
        (again ? reimport(again) : load()).then(
          (m) => (mod = m),
          (error: unknown) => {
            pending = undefined;
            failures += 1;
            again = freshUrl(error, failures) ?? again;
            throw error;
          },
        ),
      )),
  };
}

export function preload<T>(m: LazyModule<T>): void {
  m.load().catch(() => undefined);
}

export function useLazyModule<T>(m: LazyModule<T>, wanted: boolean, onFail?: (() => void) | undefined): T | undefined {
  const [, loaded] = useReducer((n: number) => n + 1, 0);
  const failed = useRef(onFail);
  useLayoutEffect(() => {
    failed.current = onFail;
  });
  const mod = m.current();
  useEffect(() => {
    if (!wanted || mod) return;
    let live = true;
    m.load().then(
      () => live && loaded(),
      () => live && failed.current?.(),
    );
    return () => {
      live = false;
    };
  }, [m, wanted, mod]);
  return wanted ? mod : undefined;
}
