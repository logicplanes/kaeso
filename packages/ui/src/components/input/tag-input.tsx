import { forwardRef, useRef, useState, type ClipboardEvent, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { Tag } from "../tag/tag.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TAG_INPUT_VARIANTS = {
  size: {
    md: { classes: "min-h-36 p-4", description: "36px at one row; grows as tags wrap (default)" },
  },
} as const;
export const KAESO_TAG_INPUT_DEFAULT_VARIANTS = { size: "md" } as const;

export interface TagInputProps extends FieldWrapProps {
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
  onInputValueChange?: ((draft: string) => void) | undefined;
  placeholder?: string | undefined;
  validate?: ((tag: string) => true | string) | undefined;
  separators?: string[] | undefined;
  disabled?: boolean | undefined;
  id?: string | undefined;
  name?: string | undefined;
  className?: string | undefined;
  "aria-label"?: string | undefined;
  "data-force"?: string | undefined;
}

export const TagInput = forwardRef<HTMLInputElement, TagInputProps>(function TagInput(props, ref) {
  const {
    value,
    defaultValue = [],
    onValueChange,
    onInputValueChange,
    placeholder,
    validate,
    separators = [",", " ", ";"],
    disabled,
    id,
    name,
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
  const [inner, setInner] = useState<string[]>(defaultValue);
  const [draft, setDraftState] = useState("");
  const setDraft = (next: string) => {
    setDraftState(next);
    onInputValueChange?.(next);
  };
  const input = useRef<HTMLInputElement | null>(null);
  const tags = value ?? inner;
  const set = (next: string[]) => {
    if (value === undefined) setInner(next);
    onValueChange?.(next);
  };
  const splitter = new RegExp(`[${separators.map((s) => s.replace(/[\]\\^-]/g, "\\$&")).join("")}\\n]+`);
  const add = (raw: string) => {
    const parts = raw
      .split(splitter)
      .map((p) => p.trim())
      .filter(Boolean)
      .filter((p) => !tags.includes(p));
    if (parts.length) set([...tags, ...parts]);
    setDraft("");
  };
  const messages = tags.map((tag) => (validate ? validate(tag) : true));
  const firstError = messages.find((m) => m !== true);
  const fieldError = error ?? (typeof firstError === "string" ? firstError : undefined);
  const control = (
    <div
      className={cn(
        "flex w-full min-w-0 flex-wrap items-center gap-4 rounded-field material-field transition-control cursor-text",
        KAESO_TAG_INPUT_VARIANTS.size.md.classes,
        disabled && "cursor-not-allowed",
        !(label || description || fieldError) && className,
      )}
      {...(fieldError || field?.invalid ? { "data-invalid": "" } : {})}
      {...(disabled ? { "data-disabled": "" } : {})}
      {...(force ? { "data-force": force } : {})}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          e.preventDefault();
          input.current?.focus();
        }
      }}
    >
      {tags.map((tag, i) => {
        const message = messages[i];
        return (
          <Tag
            key={tag}
            invalid={message !== true}
            {...(disabled ? {} : { onRemove: () => set(tags.filter((x) => x !== tag)), removeLabel: t("tagInput.remove", "Remove {tag}", { tag }) })}
          >
            {typeof message === "string" ? (
              <Tooltip content={message}>
                <span>{tag}</span>
              </Tooltip>
            ) : (
              tag
            )}
          </Tag>
        );
      })}
      <input
        ref={(el) => {
          input.current = el;
          if (typeof ref === "function") ref(el);
          else if (ref) ref.current = el;
        }}
        {...(id ? { id } : {})}
        {...fc.controlProps}
        {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
        {...(fieldError ? { "aria-invalid": true } : {})}
        disabled={disabled}
        required={required && tags.length === 0}
        {...(required ? { "aria-required": true } : {})}
        value={draft}
        placeholder={tags.length ? undefined : placeholder}
        onChange={(e) => {
          const v = e.target.value;
          if (separators.some((s) => v.endsWith(s))) add(v);
          else setDraft(v);
        }}
        onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
          if (e.key === "Enter" && draft.trim()) {
            e.preventDefault();
            add(draft);
          } else if (e.key === "Backspace" && !draft && tags.length) {
            set(tags.slice(0, -1));
          }
        }}
        onPaste={(e: ClipboardEvent<HTMLInputElement>) => {
          const text = e.clipboardData.getData("text");
          if (splitter.test(text)) {
            e.preventDefault();
            add(draft + text);
          }
        }}
        onBlur={() => draft.trim() && add(draft)}
        className={cn(
          "my-4 min-w-80 flex-1 bg-transparent px-8 text-body text-fg outline-none placeholder:text-fg-tertiary disabled:cursor-not-allowed",
          tags.length === 0 && "pl-12",
        )}
      />
      {name && <input type="hidden" name={name} value={tags.join(",")} />}
    </div>
  );
  if (field) return control;
  return withField({ label, description, error: fieldError, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
TagInput.displayName = "TagInput";
