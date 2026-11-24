import { lazy, type ComponentType } from "react";

type ReadyComponent<P> = ComponentType<P> & { preload: () => Promise<void> };

// A part split from its page that draws in the page's own frame once `preload` has brought its code. A plain `lazy`
// part suspends on its first render even when its code is in, and React reveals it a frame or more later, often
// hundreds of milliseconds, moving whatever sits below it.
export function lazyReady<P extends object>(load: () => Promise<ComponentType<P>>): ReadyComponent<P> {
  let loaded: ComponentType<P> | undefined;
  let loading: Promise<ComponentType<P>> | undefined;
  const get = () =>
    (loading ??= load().then(
      (component) => (loaded = component),
      (error: unknown) => {
        loading = undefined;
        throw error;
      },
    ));
  const Lazy = lazy(() => get().then((component) => ({ default: component })));
  function Ready(props: P) {
    const Component = (loaded ?? Lazy) as ComponentType<P>;
    return <Component {...props} />;
  }
  return Object.assign(Ready, { preload: () => get().then(() => undefined) });
}
