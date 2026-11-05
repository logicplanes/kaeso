import { forwardRef, useRef, useState, type InputHTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { warnStylingClassName } from "../../utils/dev.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { InputGroup, INPUT_ICON_SIZE, type InputSize } from "./input-group.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Kbd } from "../kbd/kbd.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_INPUT_VARIANTS = {
  size: {
    sm: { classes: "h-32", description: "32px: toolbars and compact forms" },
    md: { classes: "h-36", description: "36px: the default" },
    lg: { classes: "h-40", description: "40px" },
    xl: { classes: "h-48", description: "48px: sign-in and setup forms" },
  },
} as const;
export const KAESO_INPUT_DEFAULT_VARIANTS = { size: "md" } as const;

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size" | "prefix">, FieldWrapProps {
  size?: InputSize | undefined;
  leadingIcon?: IconData | undefined;
  prefix?: ReactNode | undefined;
  suffix?: ReactNode | undefined;
  trailing?: ReactNode | undefined;
  invalid?: boolean | undefined;
  align?: "start" | "end" | undefined;
  shortcut?: string[] | undefined;
  onClear?: (() => void) | undefined;
  secret?: boolean | undefined;
  defaultRevealed?: boolean | undefined;
  "data-force"?: string | undefined;
  className?: string | undefined;
}

type BaseProps = Omit<InputProps, "shortcut" | "onClear" | "secret" | "defaultRevealed">;

const BaseInput = forwardRef<HTMLInputElement, BaseProps>(function BaseInput(props, ref) {
  const {
    size = "md",
    leadingIcon,
    prefix,
    suffix,
    trailing,
    invalid,
    align,
    label,
    description,
    error,
    optional,
    required,
    layout,
    className,
    disabled,
    readOnly,
    id,
    "data-force": force,
    ...rest
  } = props;
  warnStylingClassName("Input", className);
  const field = useField();
  const fc = useFieldControl(props);
  const inner = (
    <InputGroupFromField
      size={size}
      leadingIcon={leadingIcon}
      prefix={prefix}
      suffix={suffix}
      trailing={trailing}
      invalid={invalid || !!error}
      disabled={disabled}
      readOnly={readOnly}
      force={force}
      className={label || description || error ? undefined : className}
    >
      <InputGroup.Input
        ref={ref}
        disabled={disabled}
        readOnly={readOnly}
        required={required}
        className={align === "end" ? "text-end tabular" : undefined}
        {...(id ? { id } : {})}
        {...fc.controlProps}
        {...(invalid || error ? { "aria-invalid": true } : {})}
        {...rest}
      />
    </InputGroupFromField>
  );
  if (field) return inner;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, inner);
});
BaseInput.displayName = "Input";

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(props, ref) {
  if (props.type === "search") return <SearchField ref={ref} {...props} />;
  if (props.type === "password" || props.secret) return <MaskedField ref={ref} {...props} />;
  const { shortcut: _shortcut, onClear: _onClear, secret: _secret, defaultRevealed: _revealed, ...rest } = props;
  return <BaseInput ref={ref} {...rest} />;
});
Input.displayName = "Input";

function InputGroupFromField({
  size,
  leadingIcon,
  prefix,
  suffix,
  trailing,
  invalid,
  disabled,
  readOnly,
  force,
  className,
  children,
}: {
  size: InputSize;
  leadingIcon?: IconData | undefined;
  prefix?: ReactNode | undefined;
  suffix?: ReactNode | undefined;
  trailing?: ReactNode | undefined;
  invalid?: boolean | undefined;
  disabled?: boolean | undefined;
  readOnly?: boolean | undefined;
  force?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}) {
  return (
    <InputGroup
      size={size}
      leadingIcon={!!leadingIcon}
      trailingButton={!!trailing}
      invalid={invalid}
      disabled={disabled}
      readOnly={readOnly}
      className={className}
      {...(force ? { "data-force": force } : {})}
    >
      {leadingIcon && <Icon icon={leadingIcon} size={INPUT_ICON_SIZE[size]} className="text-fg-tertiary" />}
      {prefix && <InputGroup.Addon data-join>{prefix}</InputGroup.Addon>}
      {children}
      {suffix && <InputGroup.Addon>{suffix}</InputGroup.Addon>}
      {trailing}
    </InputGroup>
  );
}

const SearchField = forwardRef<HTMLInputElement, InputProps>(function SearchField(
  { shortcut, onClear, value, defaultValue, onChange, onKeyDown, placeholder, secret: _secret, defaultRevealed: _revealed, ...props },
  ref,
) {
  const t = useUiText();
  const [inner, setInner] = useState(String(defaultValue ?? ""));
  const controlled = value !== undefined;
  const current = controlled ? String(value) : inner;
  const input = useRef<HTMLInputElement | null>(null);
  const clear = () => {
    const el = input.current;
    if (!controlled) setInner("");
    else if (el && !onClear) {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(el, "");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
    onClear?.();
    el?.focus();
  };
  return (
    <BaseInput
      ref={(el) => {
        input.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      leadingIcon={icons.search}
      placeholder={placeholder ?? t("searchInput.placeholder", "Search")}
      autoComplete="off"
      spellCheck={false}
      value={current}
      onChange={(e) => {
        if (!controlled) setInner(e.target.value);
        onChange?.(e);
      }}
      onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "Escape" && current) {
          e.stopPropagation();
          clear();
        }
        onKeyDown?.(e);
      }}
      trailing={
        current && !props.readOnly && !props.disabled ? (
          <InputGroup.Button icon={icons.close} aria-label={t("searchInput.clear", "Clear search")} onClick={clear} />
        ) : undefined
      }
      suffix={
        !current && shortcut ? (
          <Kbd keys={shortcut} className={props["data-force"]?.includes("focus") ? "invisible" : "group-focus-within/field:invisible"} />
        ) : undefined
      }
      {...props}
    />
  );
});
SearchField.displayName = "Input";

const MaskedField = forwardRef<HTMLInputElement, InputProps>(function MaskedField(
  { type, secret, defaultRevealed = false, shortcut: _shortcut, onClear: _onClear, onKeyDown, onKeyUp, onBlur, ...props },
  ref,
) {
  const t = useUiText();
  const password = type === "password" && !secret;
  const [revealed, setRevealed] = useState(defaultRevealed);
  const [caps, setCaps] = useState(false);
  const checkCaps = (e: KeyboardEvent<HTMLInputElement>) => setCaps(password && e.getModifierState?.("CapsLock"));
  return (
    <BaseInput
      ref={ref}
      type={revealed ? "text" : "password"}
      autoComplete={password ? "current-password" : "off"}
      autoCorrect="off"
      autoCapitalize="off"
      spellCheck={false}
      data-1p-ignore={password ? undefined : true}
      data-lpignore={password ? undefined : "true"}
      data-mono={revealed || undefined}
      onBlur={(e) => {
        setCaps(false);
        onBlur?.(e);
      }}
      onKeyDown={(e) => {
        checkCaps(e);
        onKeyDown?.(e);
      }}
      onKeyUp={(e) => {
        checkCaps(e);
        onKeyUp?.(e);
      }}
      trailing={
        <>
          {password && (
            <span className="inline-flex shrink-0 items-center gap-4 text-small text-warning-text select-none">
              {caps && <Icon icon={icons.warning} size="sm" />}
              {caps && t("sensitiveInput.capsLockShort", "Caps Lock")}
              <span role="status" className="sr-only">
                {caps ? t("sensitiveInput.capsLock", "Caps Lock is on.") : ""}
              </span>
            </span>
          )}
          <InputGroup.Button
            icon={revealed ? icons.hide : icons.show}
            aria-label={
              password
                ? revealed
                  ? t("sensitiveInput.hidePassword", "Hide password")
                  : t("sensitiveInput.showPassword", "Show password")
                : revealed
                  ? t("sensitiveInput.hideKey", "Hide key")
                  : t("sensitiveInput.showKey", "Show key")
            }
            onClick={() => setRevealed((r) => !r)}
            disabled={props.disabled}
          />
        </>
      }
      {...props}
    />
  );
});
MaskedField.displayName = "Input";
