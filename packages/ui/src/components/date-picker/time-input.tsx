import type * as React from "react";
import { forwardRef, useRef, useState, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { useControllable } from "../../utils/use-controllable.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { InputGroup, KAESO_INPUT_GROUP_VARIANTS, type InputSize } from "../input/input-group.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TIME_INPUT_VARIANTS = {
  size: KAESO_INPUT_GROUP_VARIANTS.size,
} as const;
export const KAESO_TIME_INPUT_DEFAULT_VARIANTS = { size: "md" } as const;

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
  const labelId = field ? field.labelId : label && fc.wrapId ? `${fc.wrapId}-label` : undefined;
  const [time, setTime] = useControllable(value, defaultValue, onValueChange);
  const [draft, setDraftState] = useState<{ h?: string | undefined; m?: string | undefined }>({});
  const pending = useRef(draft);
  const setDraft = (next: { h?: string | undefined; m?: string | undefined }) => {
    pending.current = next;
    setDraftState(next);
  };
  const [h = "00", m = "00"] = time.split(":");
  const minutes = useRef<HTMLInputElement>(null);
  const shown = (part: "h" | "m") => pending.current[part] ?? (part === "h" ? h : m);
  const set = (hh: number, mm: number) => {
    setDraft({});
    const next = `${pad(((hh % 24) + 24) % 24)}:${pad(((mm % 60) + 60) % 60)}`;
    if (next !== time) setTime(next);
  };
  const commit = (part: "h" | "m", v: string) => {
    const n = Number(v || 0);
    if (part === "h") set(Math.min(23, n), Number(shown("m") || 0));
    else set(Number(shown("h") || 0), Math.min(59, n));
  };
  const seg = (part: "h" | "m") => ({
    inputMode: "numeric" as const,
    maxLength: 2,
    disabled,
    value: draft[part] ?? (part === "h" ? h : m),
    onFocus: (e: React.FocusEvent<HTMLInputElement>) => e.target.select(),
    onKeyDown: (e: KeyboardEvent<HTMLInputElement>) => {
      const d = e.key === "ArrowUp" ? 1 : e.key === "ArrowDown" ? -1 : 0;
      if (d) {
        e.preventDefault();
        const hh = Number(shown("h") || 0);
        const mm = Number(shown("m") || 0);
        if (part === "h") set(hh + d, mm);
        else set(hh, mm + d);
      }
    },
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = e.target.value.replace(/\D/g, "").slice(0, 2);
      if (v.length < 2) return setDraft({ ...pending.current, [part]: v });
      commit(part, v);
      if (part === "h") minutes.current?.focus();
    },
    onBlur: () => {
      const v = pending.current[part];
      if (v !== undefined) commit(part, v);
    },
    className: "w-20 flex-none bg-transparent text-center tabular text-fg outline-none coarse:w-24",
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
      {...(labelId && !ariaLabel ? { "aria-labelledby": labelId } : { "aria-label": ariaLabel ?? t("timeInput.time", "Time") })}
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
