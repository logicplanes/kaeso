import { forwardRef, useCallback, useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";
import { warnStylingClassName } from "../../utils/dev.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";

export const KAESO_TEXTAREA_VARIANTS = {
  resize: {
    auto: { classes: "resize-none", description: "Grows with its content to 8 lines, then scrolls (default)" },
    vertical: { classes: "resize-y", description: "People can drag it taller" },
  },
} as const;
export const KAESO_TEXTAREA_DEFAULT_VARIANTS = { resize: "auto" } as const;

export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement>, FieldWrapProps {
  resize?: keyof typeof KAESO_TEXTAREA_VARIANTS.resize | undefined;
  maxRows?: number | undefined;
  invalid?: boolean | undefined;
  mono?: boolean | undefined;
  "data-force"?: string | undefined;
}

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(props, ref) {
  const {
    resize = "auto",
    maxRows = 8,
    rows = 3,
    invalid,
    mono,
    label,
    description,
    error,
    optional,
    required,
    layout,
    className,
    id,
    disabled,
    onChange,
    ...rest
  } = props;
  warnStylingClassName("Textarea", className);
  const field = useField();
  const fc = useFieldControl(props);
  const inner = useRef<HTMLTextAreaElement | null>(null);
  const grow = useCallback(() => {
    const el = inner.current;
    if (!el || resize !== "auto") return;
    const style = getComputedStyle(el);
    const line = parseFloat(style.lineHeight) || 20;
    const pad = (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0);
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, maxRows * line + pad)}px`;
  }, [maxRows, resize]);
  useLayoutEffect(grow, [grow, rest.value]);
  const control = (
    <textarea
      ref={(el) => {
        inner.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      rows={rows}
      disabled={disabled}
      required={required}
      onChange={(e) => {
        grow();
        onChange?.(e);
      }}
      className={cn(
        "block w-full min-w-0 rounded-textarea px-16 py-10 text-body text-fg material-field transition-control outline-none placeholder:text-fg-tertiary scrollbar-quiet",
        "disabled:cursor-not-allowed",
        mono && "mono text-small",
        KAESO_TEXTAREA_VARIANTS.resize[resize].classes,
        !(label || description || error) && className,
      )}
      {...(id ? { id } : {})}
      {...fc.controlProps}
      {...(invalid || error ? { "aria-invalid": true } : {})}
      {...rest}
    />
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
Textarea.displayName = "Textarea";
