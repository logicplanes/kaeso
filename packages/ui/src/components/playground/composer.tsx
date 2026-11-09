import {
  forwardRef,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ClipboardEvent,
  type DragEvent,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useUiText } from "../../i18n/context.js";
import { cn } from "../../utils/cn.js";
import { useControllable } from "../../utils/use-controllable.js";
import { icons } from "../../icons/vocabulary.js";
import { Button } from "../button/button.js";
import { Attachments, type Attachment } from "./attachments.js";

export const KAESO_COMPOSER_VARIANTS = {
  size: {
    md: {
      classes: "rounded-frame",
      description: "Text 14/20 starting two lines tall, 8px around the buttons; the outline concentric with them (16 + 8) (default)",
    },
    sm: { classes: "rounded-frame", description: "The same box starting one line tall: a chat's prompt box, a follow-up field under a thread" },
  },
} as const;
export const KAESO_COMPOSER_DEFAULT_VARIANTS = { size: "md" } as const;

const COARSE = "(pointer: coarse)";
function subscribeCoarse(onChange: () => void) {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const q = window.matchMedia(COARSE);
  q.addEventListener?.("change", onChange);
  return () => q.removeEventListener?.("change", onChange);
}
const isCoarse = () => typeof window !== "undefined" && !!window.matchMedia?.(COARSE).matches;
const useCoarsePointer = () => useSyncExternalStore(subscribeCoarse, isCoarse, () => false);

export interface ComposerProps {
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  onSubmit?: ((value: string) => void) | undefined;
  submitOn?: "enter" | "mod-enter" | undefined;
  onStop?: (() => void) | undefined;
  streaming?: boolean | undefined;
  loading?: boolean | undefined;
  disabled?: boolean | undefined;
  clearOnSubmit?: boolean | undefined;
  placeholder?: string | undefined;
  "aria-label"?: string | undefined;
  maxRows?: number | undefined;
  size?: keyof typeof KAESO_COMPOSER_VARIANTS.size | undefined;
  actions?: ReactNode | undefined;
  submitLabel?: string | undefined;
  stopLabel?: string | undefined;
  autoFocus?: boolean | undefined;
  name?: string | undefined;
  attachments?: readonly Attachment[] | undefined;
  onAttach?: ((files: File[]) => void) | undefined;
  onRemoveAttachment?: ((id: string) => void) | undefined;
  accept?: string | undefined;
  attachLabel?: string | undefined;
  "data-force"?: string | undefined;
  className?: string | undefined;
}

const files = (list: FileList | null | undefined): File[] => (list ? Array.from(list) : []);
const carriesFiles = (e: DragEvent) => Array.from(e.dataTransfer?.types ?? []).includes("Files");

export const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  {
    value,
    defaultValue = "",
    onValueChange,
    onSubmit,
    onStop,
    submitOn = "enter",
    streaming = false,
    loading = false,
    disabled = false,
    clearOnSubmit = true,
    placeholder: placeholderProp,
    "aria-label": ariaLabelProp,
    maxRows = 10,
    size = "md",
    actions,
    submitLabel: submitLabelProp,
    stopLabel: stopLabelProp,
    autoFocus,
    name,
    attachments = [],
    onAttach,
    onRemoveAttachment,
    accept,
    attachLabel: attachLabelProp,
    "data-force": force,
    className,
  },
  ref,
) {
  const t = useUiText();
  const placeholder = placeholderProp ?? t("composer.placeholder", "Write a prompt");
  const ariaLabel = ariaLabelProp ?? t("composer.prompt", "Prompt");
  const submitLabel = submitLabelProp ?? t("composer.send", "Send");
  const stopLabel = stopLabelProp ?? t("composer.stop", "Stop");
  const attachLabel = attachLabelProp ?? t("composer.attach", "Attach files");
  const [text, setText] = useControllable(value, defaultValue, onValueChange);
  const picker = useRef<HTMLInputElement | null>(null);
  const [dropping, setDropping] = useState(false);
  const attaching = !!onAttach && !disabled;
  const waitHint = useId();

  const box = useRef<HTMLTextAreaElement | null>(null);
  const minRows = size === "sm" ? 1 : 2;
  const fade = useCallback(() => {
    const el = box.current;
    if (!el) return;
    el.style.setProperty("--scroll-area-overflow-y-start", `${Math.max(0, el.scrollTop)}px`);
    el.style.setProperty("--scroll-area-overflow-y-end", `${Math.max(0, el.scrollHeight - el.clientHeight - el.scrollTop)}px`);
  }, []);
  const grow = useCallback(() => {
    const el = box.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(Math.max(el.scrollHeight, minRows * 20 + 16), maxRows * 20 + 16)}px`;
    fade();
  }, [maxRows, minRows, fade]);
  useLayoutEffect(grow, [grow, text]);

  const empty = text.trim() === "" && attachments.length === 0;
  const canSend = !disabled && !streaming && !loading && !empty;
  const send = () => {
    if (!canSend) return;
    onSubmit?.(text);
    if (clearOnSubmit && value === undefined) setText("");
  };
  const coarse = useCoarsePointer();
  const enterSends = submitOn === "enter" && !coarse;
  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter") return;
    if (e.nativeEvent.isComposing || e.keyCode === 229) return;
    const mod = e.metaKey || e.ctrlKey;
    if (!mod && (!enterSends || e.shiftKey || e.altKey)) return;
    e.preventDefault();
    send();
  };
  const onFormSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    send();
  };
  const onPaste = (e: ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = files(e.clipboardData?.files);
    if (!attaching || pasted.length === 0) return;
    if (!e.clipboardData.getData("text/plain")) e.preventDefault();
    onAttach?.(pasted);
  };
  const drop = attaching
    ? {
        onDragEnter: (e: DragEvent<HTMLFormElement>) => {
          if (carriesFiles(e)) setDropping(true);
        },
        onDragOver: (e: DragEvent<HTMLFormElement>) => {
          if (!carriesFiles(e)) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "copy";
        },
        onDragLeave: (e: DragEvent<HTMLFormElement>) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropping(false);
        },
        onDrop: (e: DragEvent<HTMLFormElement>) => {
          setDropping(false);
          const dropped = files(e.dataTransfer?.files);
          if (dropped.length === 0) return;
          e.preventDefault();
          onAttach?.(dropped);
        },
      }
    : {};

  return (
    <form
      onSubmit={onFormSubmit}
      onMouseDown={(e) => {
        if (disabled || e.button !== 0 || !e.currentTarget.contains(e.target as Node)) return;
        const control = (e.target as Element).closest("textarea, button, a, input, select, label, [role], [tabindex]");
        if (control && e.currentTarget.contains(control)) return;
        e.preventDefault();
        box.current?.focus();
      }}
      {...drop}
      {...(force ? { "data-force": force } : {})}
      {...(disabled ? { "data-disabled": "" } : {})}
      {...(dropping && attaching ? { "data-drop": "" } : {})}
      className={cn(
        "flex min-w-0 cursor-text flex-col material-composer transition-control",
        KAESO_COMPOSER_VARIANTS.size[size].classes,
        disabled && "cursor-not-allowed",
        className,
      )}
    >
      {attachments.length > 0 && <Attachments items={attachments} onRemove={disabled ? undefined : onRemoveAttachment} className="px-8 pt-8" />}
      <textarea
        ref={(el) => {
          box.current = el;
          if (typeof ref === "function") ref(el);
          else if (ref) ref.current = el;
        }}
        rows={minRows}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        onScroll={fade}
        disabled={disabled}
        placeholder={placeholder}
        aria-label={ariaLabel}
        {...(streaming && !empty ? { "aria-describedby": waitHint } : {})}
        aria-keyshortcuts={enterSends ? "Enter Meta+Enter Control+Enter" : "Meta+Enter Control+Enter"}
        {...(enterSends ? { enterKeyHint: "send" as const } : {})}
        autoFocus={autoFocus}
        {...(name ? { name } : {})}
        className="block w-full min-w-0 resize-none bg-transparent px-16 pt-12 pb-4 text-body text-fg outline-none scrollbar-quiet scroll-fade-y placeholder:text-fg-tertiary disabled:cursor-not-allowed disabled:text-fg-disabled disabled:placeholder:text-fg-disabled"
      />
      <span id={waitHint} className="sr-only">
        {t("composer.stopToSend", "A response is arriving; stop it to send this prompt.")}
      </span>
      <div className="flex min-w-0 items-center gap-8 p-8 pt-4 controls-on-fill">
        <div className="flex min-w-0 flex-1 items-center gap-4">
          {onAttach && (
            <>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                icon={icons.attach}
                aria-label={attachLabel}
                disabled={disabled}
                onClick={() => picker.current?.click()}
              />
              <input
                ref={picker}
                type="file"
                multiple
                hidden
                tabIndex={-1}
                {...(accept ? { accept } : {})}
                onChange={(e) => {
                  const picked = files(e.currentTarget.files);
                  e.currentTarget.value = "";
                  if (picked.length) onAttach?.(picked);
                }}
              />
            </>
          )}
          {actions}
        </div>
        {streaming ? (
          <Button type="button" size="sm" variant="secondary" icon={icons.stop} aria-label={stopLabel} onClick={() => onStop?.()} disabled={disabled} />
        ) : (
          <Button type="submit" size="sm" variant="primary" icon={icons.send} aria-label={submitLabel} loading={loading} disabled={!canSend && !loading} />
        )}
      </div>
    </form>
  );
});
Composer.displayName = "Composer";
