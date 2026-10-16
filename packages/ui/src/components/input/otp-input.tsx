import { forwardRef, useId, useRef, useState, type InputHTMLAttributes } from "react";
import { cn } from "../../utils/cn";
import { useField } from "../field/field";
import { useUiText } from "../../i18n/context";

export const KAESO_OTP_INPUT_VARIANTS = {
  size: {
    md: { classes: "size-48 text-title", description: "48px cells (default)" },
    sm: { classes: "size-40 text-title-sm", description: "40px cells" },
  },
} as const;
export const KAESO_OTP_INPUT_DEFAULT_VARIANTS = { size: "md" } as const;

export interface OTPInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "defaultValue" | "onChange" | "size" | "maxLength"> {
  length?: number | undefined;
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  onComplete?: ((value: string) => void) | undefined;
  invalid?: boolean | undefined;
  size?: keyof typeof KAESO_OTP_INPUT_VARIANTS.size | undefined;
  "data-force"?: string | undefined;
}

export const OTPInput = forwardRef<HTMLInputElement, OTPInputProps>(function OTPInput(
  {
    length = 6,
    value,
    defaultValue = "",
    onValueChange,
    onComplete,
    invalid,
    size = "md",
    disabled,
    className,
    id,
    "aria-label": ariaLabel,
    "data-force": force,
    ...props
  },
  ref,
) {
  const field = useField();
  const t = useUiText();
  const [inner, setInner] = useState(defaultValue);
  const [focused, setFocused] = useState(false);
  const [sel, setSel] = useState(0);
  const auto = useId();
  const input = useRef<HTMLInputElement | null>(null);
  const current = (value ?? inner).slice(0, length);
  const set = (v: string) => {
    const clean = v.replace(/\D/g, "").slice(0, length);
    if (value === undefined) setInner(clean);
    onValueChange?.(clean);
    if (clean.length === length) onComplete?.(clean);
  };
  const active = Math.min(sel, length - 1);
  const isFocused = focused || force?.includes("focus");
  return (
    <div className={cn("relative inline-flex gap-8", className)} data-invalid={invalid || field?.invalid || undefined}>
      {Array.from({ length }, (_, i) => {
        const char = current[i] ?? "";
        const isActive = isFocused && i === Math.min(current.length, length - 1) && (sel >= current.length || i === active);
        return (
          <div
            key={i}
            aria-hidden
            className={cn(
              "flex items-center justify-center rounded-cell-lg font-semibold tabular text-fg material-field transition-control",
              KAESO_OTP_INPUT_VARIANTS.size[size].classes,
              i === 2 && length === 6 && "mr-8",
            )}
            {...(invalid || field?.invalid ? { "data-invalid": "" } : {})}
            {...(disabled ? { "data-disabled": "" } : {})}
            {...(isActive ? { "data-focused": "" } : {})}
          >
            {char || (isActive ? <span className="h-20 w-1 animate-pulse-quiet bg-fg" /> : null)}
          </div>
        );
      })}
      <input
        ref={(el) => {
          input.current = el;
          if (typeof ref === "function") ref(el);
          else if (ref) ref.current = el;
        }}
        id={field?.controlId ?? id ?? `k-otp-${auto}`}
        type="text"
        inputMode="numeric"
        autoComplete="one-time-code"
        pattern="\d*"
        maxLength={length}
        value={current}
        disabled={disabled}
        aria-label={ariaLabel ?? (field ? undefined : t("otpInput.label", "Verification code"))}
        {...(field?.controlProps["aria-describedby"] ? { "aria-describedby": field.controlProps["aria-describedby"] } : {})}
        {...(invalid || field?.invalid ? { "aria-invalid": true } : {})}
        onChange={(e) => {
          set(e.target.value);
          setSel(e.target.selectionStart ?? e.target.value.length);
        }}
        onSelect={(e) => setSel((e.target as HTMLInputElement).selectionStart ?? 0)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        className="absolute inset-0 h-full w-full cursor-text bg-transparent text-transparent caret-transparent opacity-0 outline-none disabled:cursor-not-allowed"
        {...props}
      />
    </div>
  );
});
OTPInput.displayName = "OTPInput";
