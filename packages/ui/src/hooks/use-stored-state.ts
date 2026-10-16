import { useCallback, useMemo, useState, useSyncExternalStore } from "react";

let listeners: Map<string, Set<() => void>> | undefined;
let memory: Map<string, string> | undefined;

function read(key: string): string | null {
  try {
    const raw = localStorage.getItem(key);
    if (raw !== null) return raw;
  } catch {}
  return memory?.get(key) ?? null;
}

function subscribe(key: string, onChange: () => void): () => void {
  listeners ??= new Map();
  let set = listeners.get(key);
  if (!set) listeners.set(key, (set = new Set()));
  set.add(onChange);
  const onStorage = (e: StorageEvent) => {
    if (e.key === key) onChange();
  };
  window.addEventListener("storage", onStorage);
  return () => {
    set.delete(onChange);
    window.removeEventListener("storage", onStorage);
  };
}

const noSubscription = () => () => undefined;
const nothing = () => null;

export function useStoredState<T>(key: string | undefined, initial: T): [T, (v: T) => void] {
  const k = key ? `kaeso:${key}` : undefined;
  const [own, setOwn] = useState<T>(initial);
  const sub = useCallback((onChange: () => void) => (k ? subscribe(k, onChange) : () => undefined), [k]);
  const snapshot = useCallback(() => (k ? read(k) : null), [k]);
  const raw = useSyncExternalStore(k ? sub : noSubscription, snapshot, nothing);
  const stored = useMemo(() => {
    if (raw === null) return undefined;
    try {
      return { value: JSON.parse(raw) as T };
    } catch {
      return undefined;
    }
  }, [raw]);
  const set = useCallback(
    (v: T) => {
      if (!k) {
        setOwn(v);
        return;
      }
      const json = JSON.stringify(v);
      try {
        localStorage.setItem(k, json);
      } catch {
        (memory ??= new Map()).set(k, json);
      }
      listeners?.get(k)?.forEach((notify) => notify());
    },
    [k],
  );
  return [k ? (stored ? stored.value : initial) : own, set];
}
