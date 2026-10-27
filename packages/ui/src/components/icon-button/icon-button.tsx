import type * as React from "react";
import { forwardRef } from "react";
import { Button, type ButtonSize, type ButtonVariant } from "../button/button.js";
import { Tooltip } from "../tooltip/tooltip.js";
import type { IconData } from "../icon/icon.js";

export const KAESO_ICON_BUTTON_VARIANTS = {
  variant: {
    ghost: { classes: "material-ghost", description: "Toolbar and row actions (default)" },
    quiet: {
      classes: "material-ghost text-fg-secondary hover:text-fg active:text-fg open:text-fg",
      description: "Secondary text that darkens on hover: inside fields",
    },
    secondary: { classes: "material-secondary", description: "Raised circles with a ring: page-header buttons" },
    soft: { classes: "material-soft", description: "A filled circle" },
    primary: { classes: "material-primary", description: "The view's one primary action, as an icon" },
    danger: { classes: "material-danger", description: "Destructive" },
    "danger-secondary": { classes: "material-danger-secondary", description: "Destructive, quieter" },
  },
  size: {
    xs: { classes: "size-24", description: "24px circle: inside rows and chips" },
    sm: { classes: "size-32", description: "32px circle: toolbars and headers (default)" },
    md: { classes: "size-36", description: "36px circle: beside md controls" },
    lg: { classes: "size-40", description: "40px circle" },
    xl: { classes: "size-48", description: "48px circle" },
  },
} as const;
export const KAESO_ICON_BUTTON_DEFAULT_VARIANTS = { variant: "ghost", size: "sm" } as const;

export interface IconButtonProps extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  icon: IconData;
  "aria-label": string;
  variant?: Exclude<ButtonVariant, "link"> | undefined;
  size?: ButtonSize | undefined;
  shortcut?: string[] | string[][] | undefined;
  tooltip?: boolean | undefined;
  tooltipSide?: "top" | "bottom" | "left" | "right" | undefined;
  loading?: boolean | undefined;
  disabledReason?: string | undefined;
  href?: string | undefined;
  "data-force"?: string | undefined;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon, variant = "ghost", size = "sm", shortcut, tooltip = true, tooltipSide = "top", "aria-label": label, disabledReason, disabled, ...props },
  ref,
) {
  const btn = (
    <Button
      ref={ref as React.Ref<HTMLButtonElement & HTMLAnchorElement>}
      shape="circle"
      icon={icon}
      aria-label={label}
      variant={variant}
      size={size}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(disabledReason !== undefined ? { disabledReason } : {})}
      {...props}
    />
  );
  if (!tooltip || disabled) return btn;
  return (
    <Tooltip content={label} side={tooltipSide} {...(shortcut ? { shortcut } : {})}>
      {btn}
    </Tooltip>
  );
});
IconButton.displayName = "IconButton";
