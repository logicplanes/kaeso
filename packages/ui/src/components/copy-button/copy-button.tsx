import type * as React from "react";
import { forwardRef, useCallback, useLayoutEffect, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useCopy } from "../../hooks/use-copy.js";
import { useTruncated } from "../../hooks/use-truncated.js";
import { Button, type ButtonSize } from "../button/button.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_COPY_BUTTON_VARIANTS = {
  variant: {
    ghost: { classes: "material-ghost", description: "Inside fields and wells (default)" },
    secondary: { classes: "material-secondary", description: "On its own" },
  },
  size: {
    xs: { classes: "size-24", description: "24px circle: inside rows and chips" },
    sm: { classes: "size-32", description: "32px circle: toolbars and headers (default)" },
    md: { classes: "size-36", description: "36px circle: beside md controls" },
    lg: { classes: "size-40", description: "40px circle" },
    xl: { classes: "size-48", description: "48px circle" },
  },
} as const;
export const KAESO_COPY_BUTTON_DEFAULT_VARIANTS = { variant: "ghost", size: "sm" } as const;

export interface CopyButtonProps {
  value: string;
  label?: string | undefined;
  size?: ButtonSize | undefined;
  variant?: keyof typeof KAESO_COPY_BUTTON_VARIANTS.variant | undefined;
  onCopy?: ((value: string) => void) | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const CopyButton = forwardRef<HTMLButtonElement, CopyButtonProps>(function CopyButton(
  { value, label: labelProp, size = "sm", variant = "ghost", onCopy, className, ...props },
  ref,
) {
  const { copied, copy } = useCopy();
  const [hover, setHover] = useState(false);
  const t = useUiText();
  const label = labelProp ?? t("copyButton.copy", "Copy");
  const done = t("copyButton.copied", "Copied");
  return (
    <Tooltip content={copied ? done : label} open={copied || hover} onOpenChange={setHover}>
      <Button
        ref={ref as React.Ref<HTMLButtonElement & HTMLAnchorElement>}
        shape="circle"
        size={size}
        variant={variant}
        icon={copied ? icons.check : icons.copy}
        aria-label={copied ? done : label}
        className={className}
        onClick={async () => {
          if (await copy(value)) onCopy?.(value);
        }}
        {...props}
      />
    </Tooltip>
  );
});
CopyButton.displayName = "CopyButton";

export const KAESO_CLIPBOARD_TEXT_VARIANTS = {
  size: {
    sm: { classes: "h-32 pl-12 pr-0 text-small", description: "32px" },
    md: { classes: "h-36 pl-16 pr-2 text-small", description: "36px (default)" },
    lg: { classes: "h-40 pl-18 pr-4 text-small", description: "40px" },
  },
} as const;
export const KAESO_CLIPBOARD_TEXT_DEFAULT_VARIANTS = { size: "md" } as const;

export interface ClipboardTextProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  value: string;
  display?: ReactNode | undefined;
  label?: string | undefined;
  size?: keyof typeof KAESO_CLIPBOARD_TEXT_VARIANTS.size | undefined;
}

export const ClipboardText = forwardRef<HTMLDivElement, ClipboardTextProps>(function ClipboardText(
  { value, display, label, size = "md", className, ...props },
  ref,
) {
  return (
    <div
      ref={ref}
      className={cn("inline-flex min-w-0 max-w-full items-center gap-8 rounded-pill bg-fill-1", KAESO_CLIPBOARD_TEXT_VARIANTS.size[size].classes, className)}
      {...props}
    >
      <span className="mono min-w-0 flex-1 truncate text-fg select-all">{display ?? value}</span>
      <CopyButton value={value} label={label} size="sm" />
    </div>
  );
});
ClipboardText.displayName = "ClipboardText";

export interface InlineCopyTextProps extends Omit<HTMLAttributes<HTMLButtonElement>, "children" | "onCopy"> {
  value: string;
  children?: ReactNode | undefined;
  label?: string | undefined;
  onCopy?: ((value: string) => void) | undefined;
  "data-force"?: string | undefined;
}

const ICON_ROOM = 18;

function sharesLine(sibling: Node, parent: CSSStyleDeclaration): boolean {
  if (sibling.nodeType === Node.TEXT_NODE) return !!sibling.textContent?.trim();
  if (!(sibling instanceof Element)) return false;
  const style = getComputedStyle(sibling);
  if (style.display === "none" || style.position === "absolute" || style.position === "fixed") return false;
  if (parent.display.endsWith("flex")) return !parent.flexDirection.startsWith("column");
  if (parent.display === "block" || parent.display === "flow-root" || parent.display === "list-item") return style.display.startsWith("inline");
  return true;
}

function hasRoomAfter(el: Element): boolean {
  let node = el;
  for (let parent = el.parentElement; parent; node = parent, parent = parent.parentElement) {
    const style = getComputedStyle(parent);
    for (let sibling = node.nextSibling; sibling; sibling = sibling.nextSibling) if (sharesLine(sibling, style)) return false;
    if ((parseFloat(style.direction === "rtl" ? style.paddingLeft : style.paddingRight) || 0) >= ICON_ROOM) return true;
    const overflow = style.overflowX || style.overflow;
    if (overflow && overflow !== "visible") return false;
  }
  return true;
}

function useRoomAfter() {
  const [button, setButton] = useState<HTMLButtonElement | null>(null);
  const [room, setRoom] = useState(false);
  useLayoutEffect(() => {
    const parent = button?.parentElement;
    if (!button || !parent) return;
    const touch = typeof window.matchMedia === "function" ? window.matchMedia("(pointer: coarse)") : null;
    const check = () => setRoom(!touch?.matches && hasRoomAfter(button));
    check();
    const watch = new MutationObserver(check);
    watch.observe(parent, { childList: true });
    touch?.addEventListener("change", check);
    return () => {
      watch.disconnect();
      touch?.removeEventListener("change", check);
    };
  }, [button]);
  return [room, setButton] as const;
}

export const InlineCopyText = forwardRef<HTMLButtonElement, InlineCopyTextProps>(function InlineCopyText(
  { value, children, label, onCopy, className, ...props },
  ref,
) {
  const { copied, copy } = useCopy();
  const [hover, setHover] = useState(false);
  const [room, setButton] = useRoomAfter();
  const buttonRef = useCallback(
    (el: HTMLButtonElement | null) => {
      setButton(el);
      if (typeof ref === "function") ref(el);
      else if (ref) ref.current = el;
    },
    [ref, setButton],
  );
  const text = useRef<HTMLSpanElement>(null);
  const cut = useTruncated(text);
  const t = useUiText();
  const done = t("copyButton.copied", "Copied");
  return (
    <Tooltip
      content={copied ? done : cut ? value : (label ?? t("copyButton.copy", "Copy"))}
      shape={cut && !copied ? "multiline" : "pill"}
      open={copied || hover}
      onOpenChange={setHover}
    >
      <button
        ref={buttonRef}
        type="button"
        onClick={async () => {
          if (await copy(value)) onCopy?.(value);
        }}
        className={cn(
          "group/copy relative -mx-4 inline-flex max-w-full-bleed-4 min-w-0 items-center gap-4 rounded-code px-4 align-baseline mono text-small text-fg transition-control focus-ring tap-target",
          "hover:bg-fill-1 active:bg-fill-2 cursor-copy",
          className,
        )}
        {...props}
      >
        <span ref={text} className="min-w-0 truncate">
          {children ?? value}
        </span>
        <Icon
          icon={copied ? icons.check : icons.copy}
          size="xs"
          className={cn(
            room && "pointer-events-none absolute start-full group-hover/copy:pointer-events-auto group-focus-visible/copy:pointer-events-auto",
            "text-fg-tertiary opacity-0 transition-control group-hover/copy:opacity-100 coarse:opacity-100 group-focus-visible/copy:opacity-100",
            copied && "pointer-events-auto opacity-100 text-fg",
          )}
        />
        <span className="sr-only" aria-live="polite">
          {copied ? done : ""}
        </span>
      </button>
    </Tooltip>
  );
});
InlineCopyText.displayName = "InlineCopyText";
