import type * as React from "react";
import { forwardRef, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useCopy } from "../../hooks/use-copy";
import { useTruncated } from "../../hooks/use-truncated";
import { Button, type ButtonSize } from "../button/button";
import { Tooltip } from "../tooltip/tooltip";
import { Icon } from "../icon/icon";
import { icons } from "../../icons/vocabulary";
import { useUiText } from "../../i18n/context";

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
      <CopyButton value={value} label={label} size={size === "sm" ? "sm" : "sm"} />
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

export const InlineCopyText = forwardRef<HTMLButtonElement, InlineCopyTextProps>(function InlineCopyText(
  { value, children, label, onCopy, className, ...props },
  ref,
) {
  const { copied, copy } = useCopy();
  const [hover, setHover] = useState(false);
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
        ref={ref}
        type="button"
        onClick={async () => {
          if (await copy(value)) onCopy?.(value);
        }}
        className={cn(
          "group/copy relative -mx-4 inline-flex max-w-full min-w-0 items-center gap-4 rounded-code px-4 align-baseline mono text-small text-fg transition-control focus-ring tap-target",
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
            "text-fg-tertiary opacity-0 transition-control group-hover/copy:opacity-100 coarse:opacity-100 group-focus-visible/copy:opacity-100",
            copied && "opacity-100 text-fg",
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
