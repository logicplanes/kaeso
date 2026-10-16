import type { HTMLAttributes, Ref, RefCallback } from "react";

export type TriggerProps = HTMLAttributes<Element> & { ref?: Ref<Element> | undefined };

export interface TriggerPropsStore {
  get: () => TriggerProps | null;
  set: (props: TriggerProps | null) => void;
  subscribe: (listener: () => void) => () => void;
  ref: RefCallback<Element>;
}

export function triggerPropsStore(): TriggerPropsStore {
  let props: TriggerProps | null = null;
  let element: Element | null = null;
  let given: Ref<Element> | undefined;
  let detach: (() => void) | undefined;
  const listeners = new Set<() => void>();
  const attach = () => {
    detach?.();
    detach = element && given ? setRef(given, element) : undefined;
  };
  return {
    get: () => props,
    set: (next) => {
      if (next?.ref !== given) {
        given = next?.ref;
        attach();
      }
      props = next;
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    ref: (node) => {
      element = node;
      attach();
    },
  };
}

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
