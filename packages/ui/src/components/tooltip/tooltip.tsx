import { cloneElement, isValidElement, useMemo, useState, useSyncExternalStore, type ReactElement, type ReactNode, type Ref, type RefCallback } from "react";
import { cn } from "../../utils/cn";
import { lazyModule, useLazyModule } from "../../utils/lazy-module";
import { setRef, triggerPropsStore, type TriggerProps, type TriggerPropsStore } from "./trigger-props";
import { KbdSequence } from "../kbd/kbd";

export const KAESO_TOOLTIP_VARIANTS = {
  side: {
    top: { classes: "", description: "Above the trigger (default)" },
    bottom: { classes: "", description: "Below the trigger" },
    left: { classes: "", description: "Left of the trigger" },
    right: { classes: "", description: "Right of the trigger" },
  },
  shape: {
    pill: {
      classes: "rounded-pill h-32 px-12 whitespace-nowrap",
      description: "Single line: a pill (default)",
    },
    multiline: {
      classes: "rounded-tooltip px-12 py-8 max-w-280 text-pretty",
      description: "Several lines: radius 12, max 280px",
    },
  },
} as const;
export const KAESO_TOOLTIP_DEFAULT_VARIANTS = { side: "top", shape: "pill" } as const;

export interface TooltipProps {
  content: ReactNode;
  shortcut?: string[] | string[][] | undefined;
  children: ReactElement;
  side?: keyof typeof KAESO_TOOLTIP_VARIANTS.side | undefined;
  shape?: keyof typeof KAESO_TOOLTIP_VARIANTS.shape | undefined;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  disabled?: boolean | undefined;
  delay?: number | undefined;
  sideOffset?: number | undefined;
}

const tooltipRoot = lazyModule(() => import("./tooltip-root"));

export function Tooltip(props: TooltipProps) {
  const mod = useLazyModule(tooltipRoot, true);
  const [store] = useState(triggerPropsStore);
  if (!isValidElement(props.children)) return props.children;
  return (
    <>
      <Trigger store={store} merge={mod?.mergeTriggerProps}>
        {props.children}
      </Trigger>
      {mod && <mod.TooltipRoot {...props} triggerProps={store} />}
    </>
  );
}
Tooltip.displayName = "Tooltip";

function Trigger({
  store,
  merge,
  children,
}: {
  store: TriggerPropsStore;
  merge: ((given: TriggerProps, own: object) => object) | undefined;
  children: ReactElement;
}) {
  const given = useSyncExternalStore(store.subscribe, store.get, store.get);
  const own = children.props as { ref?: Ref<Element> | undefined };
  const ref = useMemo(() => mergeRefs(own.ref, store.ref), [own.ref, store]);
  return cloneElement(children as ReactElement<{ ref?: Ref<Element> | undefined }>, { ...(given && merge ? merge(given, own) : {}), ref });
}
Trigger.displayName = "Tooltip.Trigger";

function mergeRefs<T>(...refs: (Ref<T> | undefined)[]): RefCallback<T> {
  return (node) => {
    const cleanups = refs.map((ref) => setRef(ref, node));
    return () => {
      for (const cleanup of cleanups) cleanup?.();
    };
  };
}

export function TooltipSurface({
  children,
  shortcut,
  shape = "pill",
}: {
  children: ReactNode;
  shortcut?: string[] | string[][];
  shape?: keyof typeof KAESO_TOOLTIP_VARIANTS.shape;
}) {
  return (
    <div
      role="tooltip"
      className={cn("inline-flex items-center gap-8 bg-tooltip text-small font-medium text-fg shadow-raised", KAESO_TOOLTIP_VARIANTS.shape[shape].classes)}
    >
      <span>{children}</span>
      {shortcut && <KbdSequence keys={shortcut} />}
    </div>
  );
}
TooltipSurface.displayName = "TooltipSurface";
