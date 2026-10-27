import { forwardRef, useMemo, useState, type ChangeEvent, type FocusEvent, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { Input, type InputProps } from "./input.js";
import { Button } from "../button/button.js";
import { icons } from "../../icons/vocabulary.js";
import { addDecimal, compareDecimal, decimalSeparators, groupDecimal, isDecimal, localizeDecimal, ungroup } from "./decimal.js";
import { useLocale, useUiText } from "../../i18n/context.js";

export const KAESO_NUMBER_INPUT_VARIANTS = {
  align: {
    end: { classes: "text-end", description: "End-aligned tabular digits (default)" },
    start: { classes: "text-start", description: "Start-aligned" },
  },
} as const;
export const KAESO_NUMBER_INPUT_DEFAULT_VARIANTS = { align: "end" } as const;

export interface NumberInputProps extends Omit<InputProps, "type" | "value" | "defaultValue" | "onChange" | "min" | "max" | "step"> {
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  unit?: string | undefined;
  step?: string | undefined;
  min?: string | undefined;
  max?: string | undefined;
  hideStepper?: boolean | undefined;
  integer?: boolean | undefined;
  align?: keyof typeof KAESO_NUMBER_INPUT_VARIANTS.align | undefined;
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput(
  {
    value,
    defaultValue = "",
    onValueChange,
    unit,
    step = "1",
    min,
    max,
    hideStepper,
    integer,
    align = "end",
    onBlur,
    onFocus,
    onKeyDown,
    className,
    disabled,
    readOnly,
    ...props
  },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const seps = useMemo(() => decimalSeparators(locale), [locale]);
  const [inner, setInner] = useState(defaultValue);
  const [focused, setFocused] = useState(false);
  const controlled = value !== undefined;
  const raw = controlled ? value : inner;
  const set = (v: string) => {
    if (!controlled) setInner(v);
    onValueChange?.(v);
  };
  const clamp = (v: string) => {
    if (v === "" || v === "-") return v;
    if (min !== undefined && compareDecimal(v, min) < 0) return min;
    if (max !== undefined && compareDecimal(v, max) > 0) return max;
    return v;
  };
  const stepBy = (dir: 1 | -1) => set(clamp(addDecimal(raw || "0", dir === 1 ? step : `-${step}`)));
  const display = integer ? raw : focused ? localizeDecimal(raw, seps) : groupDecimal(raw, seps);
  const live = !disabled && !readOnly;
  const buttonSize = props.size === "xl" ? "md" : props.size === "sm" ? "xs" : "sm";
  const atLimit = (dir: 1 | -1) => {
    const limit = dir === 1 ? max : min;
    return limit !== undefined && isDecimal(raw) && raw !== "" && raw !== "-" && compareDecimal(raw, limit) * dir >= 0;
  };
  const stepButton = (dir: 1 | -1) => (
    <Button
      shape="circle"
      variant="quiet"
      size={buttonSize}
      tabIndex={-1}
      icon={dir === 1 ? icons.add : icons.minus}
      aria-label={dir === 1 ? t("numberInput.increase", "Increase") : t("numberInput.decrease", "Decrease")}
      onClick={() => stepBy(dir)}
      disabled={!live || atLimit(dir)}
    />
  );
  const stepper = !hideStepper && (
    <span className={cn("flex shrink-0 items-center", !live && "invisible")} {...(live ? {} : { "aria-hidden": true })}>
      {stepButton(-1)}
      {stepButton(1)}
    </span>
  );
  const numeric = raw !== "" && raw !== "-" ? Number(raw) : undefined;
  return (
    <Input
      ref={ref}
      type="text"
      role="spinbutton"
      {...(numeric !== undefined && Number.isFinite(numeric) ? { "aria-valuenow": numeric, "aria-valuetext": unit ? `${display} ${unit}` : display } : {})}
      {...(min !== undefined ? { "aria-valuemin": Number(min) } : {})}
      {...(max !== undefined ? { "aria-valuemax": Number(max) } : {})}
      inputMode={integer ? "numeric" : "decimal"}
      autoComplete="off"
      spellCheck={false}
      value={display}
      disabled={disabled}
      readOnly={readOnly}
      className={className}
      suffix={unit}
      trailing={stepper || undefined}
      onChange={(e: ChangeEvent<HTMLInputElement>) => {
        if (integer) {
          if (/^-?\d*$/.test(e.target.value)) set(e.target.value);
          return;
        }
        const v = ungroup(e.target.value, seps);
        if (isDecimal(v)) set(v);
      }}
      onFocus={(e: FocusEvent<HTMLInputElement>) => {
        setFocused(true);
        onFocus?.(e);
      }}
      onBlur={(e: FocusEvent<HTMLInputElement>) => {
        setFocused(false);
        if (raw !== clamp(raw)) set(clamp(raw));
        onBlur?.(e);
      }}
      onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "ArrowUp") {
          e.preventDefault();
          stepBy(1);
        } else if (e.key === "ArrowDown") {
          e.preventDefault();
          stepBy(-1);
        }
        onKeyDown?.(e);
      }}
      align={align}
      {...props}
    />
  );
});
NumberInput.displayName = "NumberInput";
