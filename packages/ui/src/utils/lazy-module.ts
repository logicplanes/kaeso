import { useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";

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

export interface LazyModuleOptions<T> {
  base?: (() => string) | undefined;
  reimport?: ((url: string) => Promise<T>) | undefined;
}

const CHROMIUM_FAILED_FETCH = /^Failed to fetch dynamically imported module: (\S+)$/;
const IMPORTED = /\bimport\(\s*(["'`])([^"'`$]+)\1\s*\)/;

function failedAddress(error: unknown, load: () => unknown, base: (() => string) | undefined): string | undefined {
  const named = error instanceof Error ? CHROMIUM_FAILED_FETCH.exec(error.message)?.[1] : undefined;
  const from = base?.();
  const address = named ?? (from ? IMPORTED.exec(String(load))?.[2] : undefined);
  return address && URL.canParse(address, from) ? new URL(address, from).href : undefined;
}

function fresh(address: string, attempt: number): string {
  const url = new URL(address);
  url.searchParams.set("retry", String(attempt));
  return url.href;
}

export function lazyModule<T>(load: () => Promise<T>, { base, reimport = (url) => import(url) as Promise<T> }: LazyModuleOptions<T> = {}): LazyModule<T> {
  let mod: T | undefined;
  let pending: Promise<T> | undefined;
  let failures = 0;
  let address: string | undefined;
  const attempt = () => (address ? reimport(fresh(address, failures)).catch(() => load()) : load());
  return {
    current: () => mod,
    load: () =>
      (pending ??= track(
        attempt().then(
          (m) => (mod = m),
          (error: unknown) => {
            pending = undefined;
            failures += 1;
            address = failedAddress(error, load, base) ?? address;
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

export function useLazyModuleRetried<T>(m: LazyModule<T>, wanted: boolean): [T | undefined, () => void] {
  const [failed, setFailed] = useState(false);
  const mod = useLazyModule(m, wanted && !failed, () => setFailed(true));
  return [mod, () => setFailed(false)];
}
