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
  const [armed, setArmed] = useState<number | null>(null);
  const [status, setStatus] = useState("");
  const tags = value ?? inner;
  const set = (next: string[]) => {
    if (value === undefined) setInner(next);
    onValueChange?.(next);
  };
  const splitter = new RegExp(`[${separators.map((s) => s.replace(/[\]\\^-]/g, "\\$&")).join("")}\\n\\t]+`);
  const add = (raw: string) => {
    const parts = raw
      .split(splitter)
      .map((p) => p.trim())
      .filter(Boolean)
      .filter((p) => !tags.includes(p));
    if (parts.length) set([...tags, ...parts]);
    setDraft("");
  };
  const remove = (index: number) => {
    const tag = tags[index];
    if (tag === undefined) return;
    set(tags.filter((_, i) => i !== index));
    setArmed(null);
    setStatus(t("tagInput.removed", "Removed {tag}", { tag }));
  };
  const arm = (index: number | null) => {
    setArmed(index);
    const tag = index === null ? undefined : tags[index];
    setStatus(tag === undefined ? "" : t("tagInput.selected", "{tag} selected. Press Backspace to remove it.", { tag }));
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
            {...(disabled ? { variant: "soft" as const } : {})}
            className="focus-ring"
            {...(armed === i ? { "data-force": "focus" } : {})}
            {...(disabled
              ? {}
              : {
                  onRemove: () => {
                    remove(i);
                    input.current?.focus();
                  },
                  removeLabel: t("tagInput.remove", "Remove {tag}", { tag }),
                })}
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
          if (armed !== null) arm(null);
          const v = e.target.value;
          if (separators.some((s) => v.endsWith(s))) add(v);
          else setDraft(v);
        }}
        onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
          const atStart = e.currentTarget.selectionStart === 0 && e.currentTarget.selectionEnd === 0;
          if (e.key === "Enter" && draft.trim()) {
            e.preventDefault();
            add(draft);
          } else if ((e.key === "Backspace" || e.key === "Delete") && armed !== null) {
            e.preventDefault();
            remove(armed);
          } else if (e.key === "Backspace" && atStart && tags.length) {
            e.preventDefault();
            arm(tags.length - 1);
          } else if (e.key === "ArrowLeft" && atStart && tags.length) {
            e.preventDefault();
            arm(armed === null ? tags.length - 1 : Math.max(0, armed - 1));
          } else if (e.key === "ArrowRight" && armed !== null) {
            e.preventDefault();
            arm(armed + 1 < tags.length ? armed + 1 : null);
          } else if (e.key === "Escape" && armed !== null) {
            e.stopPropagation();
            arm(null);
          }
        }}
        onPaste={(e: ClipboardEvent<HTMLInputElement>) => {
          const text = e.clipboardData.getData("text");
          if (splitter.test(text)) {
            e.preventDefault();
            add(draft + text);
          }
        }}
        onBlur={() => {
          setArmed(null);
          if (draft.trim()) add(draft);
        }}
        className={cn(
          "my-4 min-w-32 flex-1 bg-transparent px-8 text-body text-fg outline-none placeholder:text-fg-tertiary disabled:cursor-not-allowed",
          tags.length === 0 && "ps-10",
        )}
      />
      {name && <input type="hidden" name={name} value={tags.join(",")} />}
      <span role="status" className="sr-only">
        {status}
      </span>
    </div>
  );
  if (field) return control;
  return withField({ label, description, error: fieldError, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
TagInput.displayName = "TagInput";
