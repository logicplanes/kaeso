import type { Ref, RefCallback } from "react";

export function setRef<T>(ref: Ref<T> | undefined, value: T): (() => void) | undefined {
  if (typeof ref === "function") {
    const cleanup = ref(value);
    return typeof cleanup === "function" ? cleanup : () => void ref(null);
  }
  if (ref) {
    ref.current = value;
    return () => void (ref.current = null);
  }
  return undefined;
}

export function mergeRefs<T>(...refs: (Ref<T> | undefined)[]): RefCallback<T> {
  return (node) => {
    const cleanups = refs.map((ref) => setRef(ref, node));
    return () => {
      for (const cleanup of cleanups) cleanup?.();
    };
  };
}
