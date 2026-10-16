import type * as React from "react";
import { forwardRef, useRef, type HTMLAttributes, type MouseEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useTruncated } from "../../hooks/use-truncated.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { Icon, type IconData } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export const TAG_DOT = {
  "viz-1": "bg-viz-1",
  "viz-2": "bg-viz-2",
  "viz-3": "bg-viz-3",
  "viz-4": "bg-viz-4",
  "viz-5": "bg-viz-5",
  "viz-6": "bg-viz-6",
  "viz-7": "bg-viz-7",
  "viz-8": "bg-viz-8",
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
  info: "bg-info",
  neutral: "bg-fg-tertiary",
} as const;
export type TagDot = keyof typeof TAG_DOT;

export const KAESO_TAG_VARIANTS = {
  size: {
    md: { classes: "h-28 px-10 gap-6 text-small", description: "28px: metadata pills in lists and toolbars (default)" },
    sm: { classes: "h-24 px-8 gap-4 text-caption", description: "24px: inside table rows" },
  },
  variant: {
    raised: { classes: "bg-raised shadow-control text-fg", description: "Raised with a ring: metadata pills (default)" },
    outline: { classes: "bg-transparent shadow-ring text-fg-secondary", description: "Ring only: unselected toggle pills" },
    soft: { classes: "bg-fill-2 text-fg", description: "Filled: selected toggle pills" },
  },
} as const;
export const KAESO_TAG_DEFAULT_VARIANTS = { size: "md", variant: "raised" } as const;

export interface TagProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  children: ReactNode;
  size?: keyof typeof KAESO_TAG_VARIANTS.size | undefined;
  variant?: keyof typeof KAESO_TAG_VARIANTS.variant | undefined;
  icon?: IconData | undefined;
  onRemove?: ((e: MouseEvent<HTMLButtonElement>) => void) | undefined;
  removeLabel?: string | undefined;
  onClick?: ((e: MouseEvent<HTMLElement>) => void) | undefined;
  pressed?: boolean | undefined;
  invalid?: boolean | undefined;
  "data-force"?: string | undefined;
}

export const Tag = forwardRef<HTMLElement, TagProps>(function Tag(
  { children, size = "md", variant = "raised", icon, onRemove, removeLabel, onClick, pressed, invalid, className, "data-force": force, ...props },
  ref,
) {
  const t = useUiText();
  const clickable = !!onClick;
  const split = clickable && !!onRemove;
  const labelRef = useRef<HTMLSpanElement>(null);
  const truncated = useTruncated(labelRef);
  const tip = truncated && typeof children === "string";
  const label = (
    <span ref={labelRef} className="min-w-0 truncate" {...(tip && !clickable ? { tabIndex: 0 } : {})}>
      {children}
    </span>
  );
  const content = (
    <>
      {icon && <Icon icon={icon} size="xs" className="text-fg-secondary" />}
      {tip ? (
        <Tooltip content={children} shape="multiline">
          {label}
        </Tooltip>
      ) : (
        label
      )}
    </>
  );
  const forced = force ? { "data-force": force } : {};
  const press = {
    type: "button" as const,
    onClick,
    ...(pressed !== undefined ? { "aria-pressed": pressed } : {}),
  };
  const Comp = (clickable && !split ? "button" : "span") as "span";
  return (
    <Comp
      ref={ref as React.Ref<HTMLSpanElement>}
      {...(clickable && !split ? press : {})}
      {...forced}
      className={cn(
        "inline-flex max-w-full shrink-0 items-center rounded-pill font-medium whitespace-nowrap transition-control",
        KAESO_TAG_VARIANTS.size[size].classes,
        KAESO_TAG_VARIANTS.variant[variant].classes,
        icon && (size === "md" ? "pl-8" : "pl-6"),
        onRemove && "pr-2",
        clickable && !split && "relative cursor-pointer focus-ring tap-target",
        split && "pl-0",
        clickable && (variant === "soft" ? "hover:bg-fill-3 active:bg-fill-3" : "hover:bg-fill-1 active:bg-fill-2"),
        clickable && variant === "outline" && "hover:text-fg",
        invalid && "ring-invalid text-danger-text",
        className,
      )}
      {...props}
    >
      {split ? (
        <button
          {...press}
          {...forced}
          className={cn(
            "relative inline-flex h-full min-w-0 cursor-pointer items-center rounded-pill focus-ring tap-target",
            size === "md" ? "gap-6" : "gap-4",
            icon ? (size === "md" ? "pl-8" : "pl-6") : size === "md" ? "pl-10" : "pl-8",
          )}
        >
          {content}
        </button>
      ) : (
        content
      )}
      {onRemove && (
        <button
          type="button"
          aria-label={
            removeLabel ??
            (typeof children === "string" && children.trim()
              ? t("tag.removeNamed", "Remove {label}", { label: children.trimEnd() })
              : t("tag.remove", "Remove"))
          }
          onClick={(e) => {
            e.stopPropagation();
            onRemove(e);
          }}
          className={cn(
            "relative inline-flex shrink-0 items-center justify-center rounded-pill text-fg-tertiary transition-control focus-ring tap-target hover:bg-fill-2 hover:text-fg",
            size === "md" ? "size-24" : "size-20",
          )}
        >
          <Icon icon={icons.close} size="xs" />
        </button>
      )}
    </Comp>
  );
});
Tag.displayName = "Tag";

export const Pill = Tag;
