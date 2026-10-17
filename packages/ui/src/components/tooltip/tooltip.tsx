import { isValidElement, useState, type FocusEvent, type PointerEvent, type ReactElement, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { lazyModule, useLazyModuleRetried } from "../../utils/lazy-module.js";
import { HandedTrigger, triggerPropsStore } from "./trigger-props.js";
import { KbdSequence } from "../kbd/kbd.js";

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

const tooltipRoot = lazyModule(() => import("./tooltip-root.js"), { base: () => import.meta.url });

export function Tooltip(props: TooltipProps) {
  const [mod, retry] = useLazyModuleRetried(tooltipRoot, true);
  const [store] = useState(triggerPropsStore);
  if (!isValidElement(props.children)) return props.children;
  const own = props.children.props as {
    onPointerEnter?: ((e: PointerEvent<Element>) => void) | undefined;
    onFocus?: ((e: FocusEvent<Element>) => void) | undefined;
  };
  const retryOnIntent = mod
    ? undefined
    : {
        onPointerEnter: (e: PointerEvent<Element>) => {
          own.onPointerEnter?.(e);
          retry();
        },
        onFocus: (e: FocusEvent<Element>) => {
          own.onFocus?.(e);
          retry();
        },
      };
  return (
    <>
      <HandedTrigger store={store} merge={mod?.mergeTriggerProps} extra={retryOnIntent}>
        {props.children}
      </HandedTrigger>
      {mod && <mod.TooltipRoot {...props} triggerProps={store} />}
    </>
  );
}
Tooltip.displayName = "Tooltip";

export function TooltipSurface({
  children,
  shortcut,
  shape = "pill",
}: {
  children: ReactNode;
  shortcut?: string[] | string[][] | undefined;
  shape?: keyof typeof KAESO_TOOLTIP_VARIANTS.shape | undefined;
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
