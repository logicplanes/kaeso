import type * as React from "react";
import { createContext, forwardRef, useContext, type HTMLAttributes, type InputHTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";
import { focusInputOnPress } from "../../utils/field-press.js";
import { Icon, type IconData, type IconSize } from "../icon/icon.js";
import { Button } from "../button/button.js";

export const KAESO_INPUT_GROUP_VARIANTS = {
  size: {
    sm: { classes: "h-32 px-12 gap-6 text-body", description: "32px: toolbars and compact forms" },
    md: { classes: "h-36 px-16 gap-8 text-body", description: "36px: the default" },
    lg: { classes: "h-40 px-18 gap-8 text-body", description: "40px" },
    xl: { classes: "h-48 px-22 gap-8 text-body-lg", description: "48px: sign-in and setup forms" },
  },
} as const;
export const KAESO_INPUT_GROUP_DEFAULT_VARIANTS = { size: "md" } as const;
export type InputSize = keyof typeof KAESO_INPUT_GROUP_VARIANTS.size;

const LEADING: Record<InputSize, string> = { sm: "pl-10", md: "pl-14", lg: "pl-16", xl: "pl-20" };
const TRAILING_BUTTON: Record<InputSize, string> = { sm: "pr-4", md: "pr-2", lg: "pr-4", xl: "pr-6" };
export const INPUT_ICON_SIZE: Record<InputSize, IconSize> = { sm: "sm", md: "sm", lg: "md", xl: "lg" };

const PREFIX_JOIN: Record<InputSize, string> = { sm: "-me-6", md: "-me-8", lg: "-me-8", xl: "-me-8" };

const Ctx = createContext<{ size: InputSize; invalid?: boolean | undefined; disabled?: boolean | undefined }>({ size: "md" });

export interface InputGroupProps extends HTMLAttributes<HTMLDivElement> {
  size?: InputSize | undefined;
  leadingIcon?: boolean | undefined;
  trailingButton?: boolean | undefined;
  invalid?: boolean | undefined;
  disabled?: boolean | undefined;
  readOnly?: boolean | undefined;
  "data-force"?: string | undefined;
}

const Root = forwardRef<HTMLDivElement, InputGroupProps>(function InputGroup(
  { size = "md", leadingIcon, trailingButton, invalid, disabled, readOnly, className, children, ...props },
  ref,
) {
  return (
    <Ctx.Provider value={{ size, invalid, disabled }}>
      <div
        ref={ref}
        className={cn(
          "group/field relative flex w-full min-w-0 items-center rounded-pill material-field transition-control cursor-text",
          KAESO_INPUT_GROUP_VARIANTS.size[size].classes,
          leadingIcon && LEADING[size],
          trailingButton && TRAILING_BUTTON[size],
          disabled && "cursor-not-allowed",
          className,
        )}
        {...(invalid ? { "data-invalid": "" } : {})}
        {...(disabled ? { "data-disabled": "" } : {})}
        {...(readOnly ? { "data-readonly": "" } : {})}
        onMouseDown={focusInputOnPress}
        {...props}
      >
        {children}
      </div>
    </Ctx.Provider>
  );
});
Root.displayName = "InputGroup";

export interface InputGroupInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {}

const Input = forwardRef<HTMLInputElement, InputGroupInputProps>(function InputGroupInput({ className, ...props }, ref) {
  const { invalid, disabled } = useContext(Ctx);
  return (
    <input
      ref={ref}
      {...(invalid ? { "aria-invalid": true } : {})}
      {...(disabled ? { disabled: true } : {})}
      className={cn(
        "min-w-0 flex-1 bg-transparent bidi-plaintext text-inherit text-fg outline-none placeholder:text-fg-tertiary disabled:cursor-not-allowed disabled:text-fg-disabled read-only:cursor-text data-mono:mono data-mono:text-small",
        className,
      )}
      {...props}
    />
  );
});
Input.displayName = "InputGroup.Input";

export interface InputGroupAddonProps extends HTMLAttributes<HTMLSpanElement> {
  icon?: IconData | undefined;
  "data-join"?: boolean | undefined;
}

const Addon = forwardRef<HTMLSpanElement, InputGroupAddonProps>(function InputGroupAddon({ icon, className, children, "data-join": join, ...props }, ref) {
  const { size } = useContext(Ctx);
  return (
    <span
      ref={ref}
      className={cn("inline-flex shrink-0 items-center gap-4 bidi-plaintext text-fg-secondary select-none", join && PREFIX_JOIN[size], className)}
      {...props}
    >
      {icon && <Icon icon={icon} size={INPUT_ICON_SIZE[size]} />}
      {children}
    </span>
  );
});
Addon.displayName = "InputGroup.Addon";

export interface InputGroupButtonProps {
  icon: IconData;
  "aria-label": string;
  onClick?: (() => void) | undefined;
  disabled?: boolean | undefined;
  className?: string | undefined;
}

const GroupButton = forwardRef<HTMLButtonElement, InputGroupButtonProps>(function InputGroupButton({ icon, className, ...props }, ref) {
  const { size, disabled } = useContext(Ctx);
  return (
    <Button
      {...(disabled ? { disabled: true } : {})}
      ref={ref as React.Ref<HTMLButtonElement & HTMLAnchorElement>}
      shape="circle"
      variant="quiet"
      size={size === "xl" ? "md" : size === "lg" ? "sm" : size === "md" ? "sm" : "xs"}
      icon={icon}
      className={className}
      {...props}
    />
  );
});
GroupButton.displayName = "InputGroup.Button";

export const InputGroup = Object.assign(Root, { Input, Addon, Button: GroupButton });
export const useInputGroupSize = () => useContext(Ctx).size;
