import { cloneElement, useMemo, useSyncExternalStore, type HTMLAttributes, type ReactElement, type Ref, type RefCallback } from "react";
import { mergeRefs, setRef } from "../../utils/merge-refs.js";

export type TriggerProps = HTMLAttributes<Element> & { ref?: Ref<Element> | undefined };

export interface TriggerPropsStore {
  get: () => TriggerProps | null;
  set: (props: TriggerProps | null) => void;
  subscribe: (listener: () => void) => () => void;
  ref: RefCallback<Element>;
  element: () => Element | null;
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
    element: () => element,
  };
}

export function HandedTrigger({
  store,
  merge,
  extra,
  children,
}: {
  store: TriggerPropsStore;
  merge: ((given: TriggerProps, own: object) => object) | undefined;
  extra?: object | undefined;
  children: ReactElement;
}) {
  const given = useSyncExternalStore(store.subscribe, store.get, store.get);
  const own = { ...(children.props as object), ...extra } as { ref?: Ref<Element> | undefined };
  const ref = useMemo(() => mergeRefs(own.ref, store.ref), [own.ref, store]);
  return cloneElement(children as ReactElement<{ ref?: Ref<Element> | undefined }>, { ...(given && merge ? merge(given, own) : extra), ref });
}
HandedTrigger.displayName = "HandedTrigger";
