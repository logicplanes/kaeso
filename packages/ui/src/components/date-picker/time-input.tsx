import type * as React from "react";
import { forwardRef, useRef, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn";
import { useControllable } from "../../utils/use-controllable";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field";
import { InputGroup, type InputSize } from "../input/input-group";
import { Icon } from "../icon/icon";
import { icons } from "../../icons/vocabulary";
import { useUiText } from "../../i18n/context";

export const KAESO_TIME_INPUT_VARIANTS = {
  hourCycle: {
    h23: { classes: "", description: "00–23 (default)" },
  },
} as const;
export const KAESO_TIME_INPUT_DEFAULT_VARIANTS = { hourCycle: "h23" } as const;

export interface TimeInputProps extends FieldWrapProps {
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  size?: InputSize | undefined;
  disabled?: boolean | undefined;
  utc?: boolean | undefined;
  id?: string | undefined;
  className?: string | undefined;
  "aria-label"?: string | undefined;
  "data-force"?: string | undefined;
}

const pad = (n: number) => String(n).padStart(2, "0");

export const TimeInput = forwardRef<HTMLDivElement, TimeInputProps>(function TimeInput(props, ref) {
  const {
    value,
    defaultValue = "09:00",
    onValueChange,
    size = "md",
    disabled,
    utc,
    id,
    className,
    label,
    description,
    error,
    optional,
    required,
    layout,
    "aria-label": ariaLabel,
    "data-force": force,
  } = props;
  const field = useField();
  const fc = useFieldControl(props);
  const t = useUiText();
  const [time, setTime] = useControllable(value, defaultValue, onValueChange);
  const [h = "00", m = "00"] = time.split(":");
  const minutes = useRef<HTMLInputElement>(null);
  const set = (hh: number, mm: number) => setTime(`${pad(((hh % 24) + 24) % 24)}:${pad(((mm % 60) + 60) % 60)}`);
  const seg = (part: "h" | "m") => ({
    inputMode: "numeric" as const,
    maxLength: 2,
    disabled,
    value: part === "h" ? h : m,
    onFocus: (e: React.FocusEvent<HTMLInputElement>) => e.target.select(),
    onKeyDown: (e: KeyboardEvent<HTMLInputElement>) => {
      const d = e.key === "ArrowUp" ? 1 : e.key === "ArrowDown" ? -1 : 0;
      if (d) {
        e.preventDefault();
        if (part === "h") set(Number(h) + d, Number(m));
        else set(Number(h), Number(m) + d);
      }
    },
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = e.target.value.replace(/\D/g, "").slice(0, 2);
      if (part === "h") {
        const n = Math.min(23, Number(v || 0));
        setTime(`${v.length === 2 ? pad(n) : v}:${m}`);
        if (v.length === 2) minutes.current?.focus();
      } else {
        const n = Math.min(59, Number(v || 0));
        setTime(`${h}:${v.length === 2 ? pad(n) : v}`);
      }
    },
    onBlur: () => set(Number(h || 0), Number(m || 0)),
    className: "w-20 flex-none bg-transparent text-center tabular text-fg outline-none",
  });
  const control = (
    <InputGroup
      ref={ref}
      size={size}
      leadingIcon
      disabled={disabled}
      invalid={!!error}
      className={cn("w-auto", !(label || description || error) && className)}
      {...(force ? { "data-force": force } : {})}
      role="group"
      aria-label={ariaLabel ?? t("timeInput.time", "Time")}
    >
      <Icon icon={icons.time} size="sm" className="text-fg-tertiary" />
      <span className="flex items-center gap-2">
        <input {...seg("h")} aria-label={t("timeInput.hours", "Hours")} {...(id ? { id } : {})} {...fc.controlProps} />
        <span className="text-fg-tertiary">:</span>
        <input ref={minutes} {...seg("m")} aria-label={t("timeInput.minutes", "Minutes")} />
      </span>
      {utc && <InputGroup.Addon>UTC</InputGroup.Addon>}
    </InputGroup>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
TimeInput.displayName = "TimeInput";
