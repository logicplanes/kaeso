import {
  forwardRef,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { cn } from "../../utils/cn.js";
import { useField } from "../field/field.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_OTP_INPUT_VARIANTS = {
  size: {
    md: { classes: "w-48 text-title", description: "48px cells (default); they shrink, square, when the row is narrower" },
    sm: { classes: "w-40 text-title-sm", description: "40px cells" },
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
    onFocus,
    onBlur,
    onKeyDown,
    onPaste,
    onClick,
    ...props
  },
  ref,
) {
  const field = useField();
  const t = useUiText();
  const [inner, setInner] = useState(defaultValue);
  const [focused, setFocused] = useState(false);
  const current = (value ?? inner).slice(0, length);
  const [sel, setSel] = useState(current.length);
  const auto = useId();
  const input = useRef<HTMLInputElement | null>(null);
  const cells = useRef<(HTMLDivElement | null)[]>([]);
  const pending = useRef<number | null>(null);
  const set = (v: string) => {
    const clean = v.replace(/\D/g, "").slice(0, length);
    if (value === undefined) setInner(clean);
    onValueChange?.(clean);
    if (clean.length === length) onComplete?.(clean);
    return clean;
  };
  const select = (i: number, text = current) => {
    const at = Math.max(0, Math.min(i, text.length, length - 1));
    input.current?.setSelectionRange(at, at < text.length ? at + 1 : at);
    setSel(at);
  };
  useLayoutEffect(() => {
    if (pending.current === null) return;
    const at = pending.current;
    pending.current = null;
    if (document.activeElement === input.current) select(at);
  });
  const isFocused = focused || !!force?.includes("focus");
  const caret = isFocused ? Math.min(sel, length - 1) : -1;
  const edit = (start: number, end: number, text: string) => {
    const digits = text.replace(/\D/g, "");
    const next = digits.length >= length ? set(digits) : set(current.slice(0, start) + digits + current.slice(end));
    pending.current = Math.min(digits.length >= length ? next.length : start + digits.length, next.length);
  };
  return (
    <div className={cn("relative inline-flex max-w-full gap-8", className)} data-invalid={invalid || field?.invalid || undefined}>
      {Array.from({ length }, (_, i) => {
        const char = current[i] ?? "";
        const isActive = i === caret;
        return (
          <div
            key={i}
            ref={(el) => {
              cells.current[i] = el;
            }}
            aria-hidden
            className={cn(
              "flex aspect-square min-w-0 shrink items-center justify-center rounded-cell-lg font-semibold tabular text-fg material-field transition-control",
              KAESO_OTP_INPUT_VARIANTS.size[size].classes,
              i === 2 && length === 6 && "mr-8",
            )}
            {...(invalid || field?.invalid ? { "data-invalid": "" } : {})}
            {...(disabled ? { "data-disabled": "" } : {})}
            {...(isActive ? { "data-focused": "" } : {})}
          >
            {char || (isActive ? <span className="h-20 w-1 bg-fg forced-line" /> : null)}
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
        value={current}
        disabled={disabled}
        aria-label={ariaLabel ?? (field ? undefined : t("otpInput.label", "Verification code"))}
        {...(field?.controlProps["aria-describedby"] ? { "aria-describedby": field.controlProps["aria-describedby"] } : {})}
        {...(invalid || field?.invalid ? { "aria-invalid": true } : {})}
        onChange={(e) => {
          const typed = e.target.value;
          const at = e.target.selectionStart ?? typed.length;
          const next = set(typed);
          pending.current = Math.min(typed.slice(0, at).replace(/\D/g, "").length, next.length);
        }}
        onPaste={(e: ClipboardEvent<HTMLInputElement>) => {
          e.preventDefault();
          const el = e.currentTarget;
          edit(el.selectionStart ?? current.length, el.selectionEnd ?? current.length, e.clipboardData.getData("text"));
          onPaste?.(e);
        }}
        onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
          const start = e.currentTarget.selectionStart ?? current.length;
          const moves: Record<string, number> = { ArrowLeft: start - 1, ArrowRight: start + 1, Home: 0, End: current.length };
          const to = moves[e.key];
          if (to !== undefined && !e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) {
            e.preventDefault();
            select(to);
          }
          onKeyDown?.(e);
        }}
        onClick={(e: MouseEvent<HTMLInputElement>) => {
          const hit = cells.current.findIndex((c) => {
            const r = c?.getBoundingClientRect();
            return !!r && r.width > 0 && e.clientX >= r.left - 4 && e.clientX <= r.right + 4;
          });
          select(hit < 0 ? current.length : hit);
          onClick?.(e);
        }}
        onSelect={(e) => setSel((e.target as HTMLInputElement).selectionStart ?? 0)}
        onFocus={(e) => {
          setFocused(true);
          onFocus?.(e);
          select(current.length);
        }}
        onBlur={(e) => {
          setFocused(false);
          onBlur?.(e);
        }}
        className="absolute inset-0 h-full w-full cursor-text bg-transparent text-transparent caret-transparent opacity-0 outline-none disabled:cursor-not-allowed"
        {...props}
      />
    </div>
  );
});
OTPInput.displayName = "OTPInput";
