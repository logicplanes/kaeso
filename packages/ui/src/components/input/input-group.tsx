import type * as React from "react";
import { createContext, forwardRef, useContext, type HTMLAttributes, type InputHTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";
import { focusInputOnPress } from "../../utils/field-press.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Button } from "../button/button.js";
import { INPUT_ICON_SIZE, KAESO_INPUT_GROUP_VARIANTS, LEADING, type InputSize } from "./sizes.js";

export { INPUT_ICON_SIZE, KAESO_INPUT_GROUP_VARIANTS, LEADING, type InputSize };

const TRAILING_BUTTON: Record<InputSize, string> = { sm: "pr-4", md: "pr-2", lg: "pr-4", xl: "pr-6" };

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
        "min-w-0 flex-1 bg-transparent bidi-plaintext text-inherit text-ellipsis text-fg outline-none placeholder:text-fg-tertiary disabled:cursor-not-allowed disabled:text-fg-disabled read-only:cursor-text data-mono:mono data-mono:text-small",
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
      tooltip={false}
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
