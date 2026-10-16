import { forwardRef, useRef, type ElementType, type HTMLAttributes, type ReactNode, type Ref } from "react";
import { cn } from "../../utils/cn";
import { useTruncated } from "../../hooks/use-truncated";
import { Tooltip } from "../tooltip/tooltip";

export const KAESO_TEXT_VARIANTS = {
  variant: {
    micro: { classes: "text-micro font-medium", description: "11/14 500: keyboard keys and chart ticks only" },
    caption: { classes: "text-caption font-medium", description: "12/16 500: group labels, badges, table headers" },
    small: { classes: "text-small", description: "13/18: descriptions, secondary cells, hints" },
    body: { classes: "text-body", description: "14/20: the default" },
    "body-lg": { classes: "text-body-lg font-medium", description: "15/22 500: settings row titles, dialog body" },
    "title-sm": { classes: "text-title-sm font-semibold", description: "17/24 600: section and dialog titles" },
    title: { classes: "text-title font-semibold", description: "20/28 600: empty-state and compact page titles" },
    "title-lg": { classes: "text-title-lg font-semibold", description: "28/34 600: settings page titles, auth screens" },
    display: { classes: "text-display font-semibold", description: "40/44 600: showcase headings, the setup screen" },
    metric: { classes: "text-metric font-semibold tabular", description: "28/32 600 tabular: big numbers" },
  },
  tone: {
    default: { classes: "text-fg", description: "Primary text" },
    secondary: { classes: "text-fg-secondary", description: "Descriptions and metadata" },
    tertiary: { classes: "text-fg-tertiary", description: "Placeholders and redundant hints; never the only carrier of meaning" },
    danger: { classes: "text-danger-text", description: "Errors, next to an icon or a word" },
    success: { classes: "text-success-text", description: "Confirmation, next to an icon or a word" },
    warning: { classes: "text-warning-text", description: "Warnings, next to an icon or a word" },
    info: { classes: "text-info-text", description: "Information in progress (syncing, queued), next to an icon or a word" },
    inherit: { classes: "text-inherit", description: "Inherit from the parent" },
  },
  weight: {
    regular: { classes: "font-regular", description: "400" },
    medium: { classes: "font-medium", description: "500 (480 in dark mode)" },
    semibold: { classes: "font-semibold", description: "600 (580 in dark mode)" },
  },
} as const;
export const KAESO_TEXT_DEFAULT_VARIANTS = { variant: "body", tone: "default" } as const;

export type TextVariant = keyof typeof KAESO_TEXT_VARIANTS.variant;
export type TextTone = keyof typeof KAESO_TEXT_VARIANTS.tone;
export type TextWeight = keyof typeof KAESO_TEXT_VARIANTS.weight;

export interface TextProps extends HTMLAttributes<HTMLElement> {
  variant?: TextVariant | undefined;
  tone?: TextTone | undefined;
  weight?: TextWeight | undefined;
  mono?: boolean | undefined;
  tabular?: boolean | undefined;
  truncate?: boolean | undefined;
  as?: ElementType | undefined;
  children?: ReactNode | undefined;
}

function mergeRefs<T>(...refs: (Ref<T> | undefined)[]) {
  return (value: T | null) => {
    for (const r of refs) {
      if (typeof r === "function") r(value);
      else if (r && typeof r === "object") (r as { current: T | null }).current = value;
    }
  };
}

export const Text = forwardRef<HTMLElement, TextProps>(function Text(
  { variant = "body", tone = "default", weight, mono, tabular, truncate, as, className, children, ...props },
  ref,
) {
  const Comp = (as ?? "span") as ElementType;
  const inner = useRef<HTMLElement>(null);
  const isTruncated = useTruncated(inner, !!truncate);
  const el = (
    <Comp
      ref={mergeRefs(ref, inner)}
      className={cn(
        KAESO_TEXT_VARIANTS.variant[variant].classes,
        KAESO_TEXT_VARIANTS.tone[tone].classes,
        weight && KAESO_TEXT_VARIANTS.weight[weight].classes,
        mono && "mono",
        mono && variant === "body" && "text-small",
        tabular && "tabular",
        truncate && "block min-w-0 truncate",
        className,
      )}
      {...(truncate && isTruncated ? { tabIndex: 0 } : {})}
      {...props}
    >
      {children}
    </Comp>
  );
  if (truncate && isTruncated && typeof children === "string") {
    return (
      <Tooltip content={children} shape="multiline">
        {el}
      </Tooltip>
    );
  }
  return el;
});
Text.displayName = "Text";
