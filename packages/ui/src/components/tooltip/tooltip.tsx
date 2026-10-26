import {
  forwardRef,
  isValidElement,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type PointerEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { usePortalContainer } from "../../utils/portal-context.js";
import { joinIds } from "../../utils/hidden-description.js";
import { cn } from "../../utils/cn.js";
import { lazyModule, useLazyModuleRetried } from "../../utils/lazy-module.js";
import { HandedTrigger, triggerPropsStore, type TriggerPropsStore } from "./trigger-props.js";
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
      classes: "rounded-pill h-28 px-12 whitespace-nowrap",
      description: "Single line: a pill (default)",
    },
    multiline: {
      classes: "rounded-tooltip px-12 py-8 max-w-280 text-pretty wrap-anywhere",
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

const words = (text: string | null | undefined) => (text ?? "").replace(/\s+/g, " ").trim().toLowerCase();

function accessibleName(el: Element): string {
  const labelledBy = el.getAttribute("aria-labelledby");
  if (labelledBy)
    return labelledBy
      .split(/\s+/)
      .map((id) => el.ownerDocument.getElementById(id)?.textContent ?? "")
      .join(" ");
  return el.getAttribute("aria-label") ?? el.textContent ?? "";
}

function ownDescription(el: Element, skip: string): string {
  const ids = (el.getAttribute("aria-describedby") ?? "").split(/\s+/).filter((id) => id && id !== skip);
  if (!ids.length) return el.getAttribute("aria-description") ?? "";
  return ids.map((id) => el.ownerDocument.getElementById(id)?.textContent ?? "").join(" ");
}

function useDescription(content: ReactNode, store: TriggerPropsStore, off: boolean) {
  const id = useId();
  const container = usePortalContainer();
  const node = useRef<HTMLSpanElement>(null);
  const [mounted, setMounted] = useState(false);
  const [describes, setDescribes] = useState(false);
  const plain = typeof content === "string" || typeof content === "number" ? String(content) : null;
  useEffect(() => setMounted(true), []);
  useLayoutEffect(() => {
    const trigger = store.element();
    const text = words(plain ?? node.current?.textContent);
    const next = !off && !!trigger && text !== "" && text !== words(accessibleName(trigger)) && !words(ownDescription(trigger, id)).includes(text);
    if (next !== describes) setDescribes(next);
  }, [plain, off, mounted, content, describes, store, id]);
  const hidden =
    mounted && (plain === null || describes) && typeof document !== "undefined"
      ? createPortal(
          <span ref={node} id={id} hidden>
            {content}
          </span>,
          container ?? document.body,
        )
      : null;
  return { id: describes ? id : undefined, node: hidden };
}

export function Tooltip(props: TooltipProps) {
  const [mod, retry] = useLazyModuleRetried(tooltipRoot, true);
  const [store] = useState(triggerPropsStore);
  const description = useDescription(props.content, store, !!props.disabled);
  if (!isValidElement(props.children)) return props.children;
  const own = props.children.props as {
    onPointerEnter?: ((e: PointerEvent<Element>) => void) | undefined;
    onFocus?: ((e: FocusEvent<Element>) => void) | undefined;
    "aria-describedby"?: string | undefined;
  };
  const describedBy = description.id ? { "aria-describedby": joinIds(own["aria-describedby"], description.id) } : undefined;
  const retryOnIntent = mod
    ? describedBy
    : {
        ...describedBy,
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
      {description.node}
      {mod && <mod.TooltipRoot {...props} triggerProps={store} />}
    </>
  );
}
Tooltip.displayName = "Tooltip";

export interface TooltipSurfaceProps {
  children: ReactNode;
  shortcut?: string[] | string[][] | undefined;
  shape?: keyof typeof KAESO_TOOLTIP_VARIANTS.shape | undefined;
}

export const TooltipSurface = forwardRef<HTMLDivElement, TooltipSurfaceProps>(function TooltipSurface({ children, shortcut, shape = "pill" }, ref) {
  return (
    <div
      ref={ref}
      role="tooltip"
      className={cn("inline-flex items-center gap-8 bg-tooltip text-small font-medium text-fg shadow-raised", KAESO_TOOLTIP_VARIANTS.shape[shape].classes)}
    >
      <span>{children}</span>
      {shortcut && <KbdSequence keys={shortcut} />}
    </div>
  );
});
TooltipSurface.displayName = "TooltipSurface";
