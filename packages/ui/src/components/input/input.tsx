import { forwardRef, useRef, useState, type InputHTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { warnStylingClassName } from "../../utils/dev.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { InputGroup, INPUT_ICON_SIZE, type InputSize } from "./input-group.js";
import { Icon, type IconData } from "../icon/icon.js";
import { KbdSequence } from "../kbd/kbd.js";
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
  "data-force"?: string | undefined;
  className?: string | undefined;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(props, ref) {
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
        className={align === "end" ? "text-right tabular" : undefined}
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
      {prefix && <InputGroup.Addon>{prefix}</InputGroup.Addon>}
      {children}
      {suffix && <InputGroup.Addon>{suffix}</InputGroup.Addon>}
      {trailing}
    </InputGroup>
  );
}

export const KAESO_SEARCH_INPUT_VARIANTS = {
  size: {
    sm: { classes: KAESO_INPUT_VARIANTS.size.sm.classes, description: "32px: toolbars (default)" },
    md: { classes: KAESO_INPUT_VARIANTS.size.md.classes, description: "36px: a search at the top of a page or dialog" },
    lg: { classes: KAESO_INPUT_VARIANTS.size.lg.classes, description: "40px" },
    xl: { classes: KAESO_INPUT_VARIANTS.size.xl.classes, description: "48px" },
  },
} as const;
export const KAESO_SEARCH_INPUT_DEFAULT_VARIANTS = { size: "sm" } as const;

export interface SearchInputProps extends Omit<InputProps, "leadingIcon" | "type" | "prefix" | "suffix"> {
  shortcut?: string[] | undefined;
  onClear?: (() => void) | undefined;
  placeholder?: string | undefined;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(function SearchInput(
  { shortcut, onClear, value, defaultValue, onChange, onKeyDown, placeholder, size = "sm", ...props },
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
    <Input
      ref={(el) => {
        input.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      type="search"
      size={size}
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
      trailing={current ? <InputGroup.Button icon={icons.close} aria-label={t("searchInput.clear", "Clear search")} onClick={clear} /> : undefined}
      suffix={!current && shortcut ? <KbdSequence keys={shortcut} /> : undefined}
      {...props}
    />
  );
});
SearchInput.displayName = "SearchInput";
