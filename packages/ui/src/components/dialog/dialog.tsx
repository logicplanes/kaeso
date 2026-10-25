import { GutterContent, InsetContent } from "../../utils/inset.js";
import { usePortalContainer } from "../../utils/portal-context.js";
import {
  cloneElement,
  createContext,
  forwardRef,
  use,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
} from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { AlertDialog as BaseAlertDialog } from "@base-ui/react/alert-dialog";
import { cn } from "../../utils/cn.js";
import { useRestoreFocus } from "../../utils/restore-focus.js";
import { useScopedDialog } from "../../utils/dialog-scope.js";
import { OverNonModalSheet } from "../../utils/over-sheet.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Button } from "../button/button.js";
import { Input } from "../input/input.js";
import { focusDialogSection, initialDialogControl } from "./focus.js";
import { useResizeMotion } from "../../utils/resize-motion.js";
import { mergeRefs } from "../../utils/merge-refs.js";

export const KAESO_DIALOG_VARIANTS = {
  size: {
    sm: { classes: "w-440", description: "440px: confirmations, short forms" },
    md: { classes: "w-560", description: "560px: forms (default)" },
    lg: { classes: "w-720", description: "720px: two columns, previews" },
  },
  anchor: {
    center: { classes: "items-center", description: "Centered on the screen (default)" },
    top: { classes: "items-start pt-dialog-top", description: "A fixed top edge, for content that changes height, such as the steps of a StepDialog" },
  },
} as const;
export const KAESO_DIALOG_DEFAULT_VARIANTS = { size: "md", anchor: "center" } as const;

const BACKDROP = "fixed inset-0 z-50 bg-overlay backdrop-motion";
const DIALOG_VIEWPORT = "fixed inset-0 z-50 flex justify-center p-16";
const DIALOG_POPUP = "relative flex dialog-limits flex-col overflow-hidden rounded-dialog bg-raised text-fg shadow-dialog outline-none dialog-motion";

const DialogBusy = createContext(false);

function useScrolledUnder<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [under, setUnder] = useState(false);
  const check = useCallback(() => {
    const el = ref.current;
    if (el && !el.closest("[data-resizing]")) setUnder(el.scrollHeight - el.scrollTop - el.clientHeight > 1);
  }, []);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    check();
    el.addEventListener("scroll", check, { passive: true });
    window.addEventListener("resize", check);
    return () => {
      el.removeEventListener("scroll", check);
      window.removeEventListener("resize", check);
    };
  }, [check]);
  return { ref, under, check };
}

function useKeepFocus(box: HTMLElement | null, inline: boolean | undefined) {
  const rendered = useRef(false);
  useEffect(() => {
    const first = !rendered.current;
    rendered.current = true;
    const active = document.activeElement;
    if (first || inline || !box || (active && active !== document.body) || box.matches("[data-ending-style], [data-closed]")) return;
    const frame = box.querySelector<HTMLElement>("[data-dialog-frame]");
    if (frame) initialDialogControl(frame)?.focus({ preventScroll: true });
  });
}

const SINGLE_LINE = new Set(["text", "email", "password", "tel", "url", "number"]);

function submitsOnEnter(e: KeyboardEvent): boolean {
  // Safari ends composition before the Enter that confirms it, leaving only keyCode 229: https://bugs.webkit.org/show_bug.cgi?id=165004
  if (e.key !== "Enter" || e.altKey || e.nativeEvent.isComposing || e.keyCode === 229) return false;
  if (e.metaKey || e.ctrlKey) return true;
  const target = e.target as HTMLElement;
  return !e.defaultPrevented && !e.shiftKey && target instanceof HTMLInputElement && SINGLE_LINE.has(target.type) && target.getAttribute("role") !== "combobox";
}

function useBox(inline: boolean | undefined) {
  const [box, setBox] = useState<HTMLElement | null>(null);
  const ref = useCallback((el: HTMLElement | null) => setBox(inline ? el : (el?.parentElement ?? null)), [inline]);
  return [box, ref] as const;
}

export interface DialogProps {
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  trigger?: ReactElement | undefined;
  title: ReactNode;
  description?: ReactNode | undefined;
  busy?: boolean | undefined;
  size?: keyof typeof KAESO_DIALOG_VARIANTS.size | undefined;
  anchor?: keyof typeof KAESO_DIALOG_VARIANTS.anchor | undefined;
  children?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  onSubmit?: (() => void) | undefined;
  hideClose?: boolean | undefined;
  headerStart?: ReactNode | undefined;
  className?: string | undefined;
  inline?: boolean | undefined;
  persistent?: boolean | undefined;
}

function DialogFrame({
  title,
  description,
  children,
  footer,
  onSubmit,
  hideClose,
  headerStart,
  size = "md",
  anchor = "center",
  className,
  inline,
  busy,
  rootRef,
}: DialogProps & { rootRef?: Ref<HTMLDivElement> | undefined }) {
  const { ref, under, check } = useScrolledUnder<HTMLDivElement>();
  const [box, boxRef] = useBox(inline);
  const frameRef = useMemo(() => mergeRefs(boxRef, rootRef), [boxRef, rootRef]);
  useResizeMotion(box, check);
  useKeepFocus(box, inline);
  const t = useUiText();
  const Title = inline ? "h2" : BaseDialog.Title;
  const closeButton = <IconButton icon={icons.close} aria-label={t("dialog.close", "Close")} tooltip={false} disabled={!!busy} />;
  const close = hideClose ? null : inline ? closeButton : <BaseDialog.Close render={closeButton} />;
  const Description = inline ? "p" : BaseDialog.Description;
  return (
    <DialogBusy value={!!busy}>
      <div
        ref={frameRef}
        data-dialog-frame=""
        className={cn(
          "flex min-h-0 flex-col",
          inline ? "rounded-dialog bg-raised shadow-dialog" : cn("flex-1 self-center", anchor === "top" ? "dialog-limits-top" : "dialog-limits"),
          KAESO_DIALOG_VARIANTS.size[size].classes,
          className,
        )}
        onKeyDown={(e: KeyboardEvent) => {
          if (onSubmit && !busy && submitsOnEnter(e)) {
            e.preventDefault();
            onSubmit();
          }
        }}
      >
        {headerStart !== undefined ? (
          <>
            <div className="flex h-32 items-center justify-between px-24 pt-16 box-content">
              <span className="flex items-center">{headerStart}</span>
              {close}
            </div>
            <div className="flex flex-col gap-4 px-24 pt-8 pb-4">
              <Title className="text-title-sm font-semibold text-fg text-balance">{title}</Title>
              {description && <Description className="text-body text-fg-secondary text-pretty">{description}</Description>}
            </div>
          </>
        ) : (
          <div className="flex items-start gap-12 px-24 pt-20 pb-4">
            <div className="flex min-w-0 flex-1 flex-col gap-4 pt-4">
              <Title className="text-title-sm font-semibold text-fg text-balance">{title}</Title>
              {description && <Description className="text-body text-fg-secondary text-pretty">{description}</Description>}
            </div>
            {close && <span className="flex">{close}</span>}
          </div>
        )}
        {children && (
          <div ref={ref} data-dialog-body="" className={cn("min-h-0 flex-1 overflow-y-auto px-24 pt-16 scrollbar-quiet", footer ? "pb-8" : "pb-24")}>
            <GutterContent>{children}</GutterContent>
          </div>
        )}
        {footer && (
          <div
            data-dialog-footer=""
            className={cn("flex shrink-0 flex-wrap items-center justify-end gap-8 px-24 pt-16 pb-20 transition-control", under && "hairline-t")}
          >
            {footer}
          </div>
        )}
      </div>
    </DialogBusy>
  );
}

function focusFirstControl(): HTMLElement | true {
  const open = [...document.querySelectorAll<HTMLElement>("[role=dialog], [role=alertdialog]")].pop();
  const frame = open?.querySelector<HTMLElement>("[data-dialog-frame]");
  return (frame && initialDialogControl(frame)) ?? open ?? true;
}

const Root = forwardRef<HTMLDivElement, DialogProps>(function Dialog(props, ref) {
  const { open, defaultOpen, onOpenChange, trigger, size = "md", anchor = "center", inline, busy, persistent } = props;
  const focus = useRestoreFocus(open, focusFirstControl);
  const container = usePortalContainer();
  const overSheet = use(OverNonModalSheet);
  const settled = useScopedDialog(inline ? false : open);
  if (inline) return <DialogFrame {...props} rootRef={ref} />;
  return (
    <BaseDialog.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      onOpenChange={(o: boolean, details: BaseDialog.Root.ChangeEventDetails) => {
        if (!o && busy) return details.cancel();
        onOpenChange?.(o);
      }}
      onOpenChangeComplete={settled}
      {...(persistent ? { disablePointerDismissal: true } : {})}
    >
      {trigger && <BaseDialog.Trigger render={trigger} />}
      <BaseDialog.Portal container={container}>
        <BaseDialog.Backdrop className={BACKDROP} forceRender={overSheet} />
        <BaseDialog.Viewport className={cn(DIALOG_VIEWPORT, KAESO_DIALOG_VARIANTS.anchor[anchor].classes)}>
          <BaseDialog.Popup
            ref={ref}
            initialFocus={focus.initialFocus}
            finalFocus={focus.finalFocus}
            className={cn(DIALOG_POPUP, KAESO_DIALOG_VARIANTS.size[size].classes, anchor === "top" && "dialog-limits-top")}
          >
            <InsetContent inset={false}>
              <OverNonModalSheet value={false}>
                <DialogFrame {...props} />
              </OverNonModalSheet>
            </InsetContent>
          </BaseDialog.Popup>
        </BaseDialog.Viewport>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
});
Root.displayName = "Dialog";

function Close({ children }: { children: ReactElement<{ disabled?: boolean | undefined }> }) {
  const busy = use(DialogBusy);
  return <BaseDialog.Close render={busy ? cloneElement(children, { disabled: true }) : children} />;
}
Close.displayName = "Dialog.Close";

export const Dialog = Object.assign(Root, { Close });

export const KAESO_ALERT_DIALOG_VARIANTS = {
  tone: {
    danger: { classes: "bg-danger-tint text-danger-text", description: "Destructive (default)" },
    warning: { classes: "bg-warning-tint text-warning-text", description: "Risky but reversible" },
    info: { classes: "bg-info-tint text-info-text", description: "Neutral confirmation" },
  },
} as const;
export const KAESO_ALERT_DIALOG_DEFAULT_VARIANTS = { tone: "danger" } as const;

export interface AlertDialogProps {
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  trigger?: ReactElement | undefined;
  title: ReactNode;
  description: ReactNode;
  confirmLabel: string;
  cancelLabel?: string | undefined;
  onConfirm: () => void | Promise<unknown>;
  tone?: keyof typeof KAESO_ALERT_DIALOG_VARIANTS.tone | undefined;
  confirmText?: string | undefined;
  loading?: boolean | undefined;
  inline?: boolean | undefined;
}

function AlertFrame({
  title,
  description,
  confirmLabel,
  cancelLabel: cancelProp,
  onConfirm,
  tone = "danger",
  confirmText,
  loading,
  inline,
  close,
  busy,
  setBusy,
  rootRef,
}: AlertDialogProps & { close: () => void; busy: boolean; setBusy: (busy: boolean) => void; rootRef?: Ref<HTMLDivElement> | undefined }) {
  const [typed, setTyped] = useState("");
  const pending = useRef(false);
  const t = useUiText();
  const [box, boxRef] = useBox(inline);
  const frameRef = useMemo(() => mergeRefs(boxRef, rootRef), [boxRef, rootRef]);
  useResizeMotion(box);
  const cancelLabel = cancelProp ?? t("alertDialog.cancel", "Cancel");
  const [typeBefore, typeAfter = ""] = t("alertDialog.typeToConfirm", "Type {text} to confirm").split("{text}");
  const Title = inline ? "h2" : BaseAlertDialog.Title;
  const Description = inline ? "p" : BaseAlertDialog.Description;
  const ok = !confirmText || typed === confirmText;
  const confirm = async () => {
    if (!ok || loading || pending.current) return;
    const r = onConfirm();
    if (!(r instanceof Promise)) return close();
    pending.current = true;
    setBusy(true);
    const done = await r.then(
      () => true,
      () => false,
    );
    pending.current = false;
    setBusy(false);
    if (done) close();
  };
  return (
    <div ref={frameRef} className={cn("flex w-440 flex-col gap-20 p-24", inline ? "rounded-dialog bg-raised shadow-dialog" : "self-center dialog-limits")}>
      <div className="flex flex-col gap-6">
        <Title className="text-title-sm font-semibold text-fg text-balance">{title}</Title>
        <Description className="text-body text-fg-secondary text-pretty">{description}</Description>
      </div>
      {confirmText && (
        <Input
          label={
            <span className="font-regular text-fg-secondary">
              {typeBefore}
              <span className="mono font-medium text-fg">{confirmText}</span>
              {typeAfter}
            </span>
          }
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          onKeyDown={(e) => e.key === "Enter" && confirm()}
        />
      )}
      <div className="flex flex-wrap justify-end gap-8">
        {inline ? <Button>{cancelLabel}</Button> : <BaseAlertDialog.Close render={<Button disabled={busy || !!loading}>{cancelLabel}</Button>} />}
        <Button variant={tone === "danger" ? "danger" : "primary"} onClick={confirm} disabled={!ok} loading={busy || loading}>
          {confirmLabel}
        </Button>
      </div>
    </div>
  );
}

export const AlertDialog = forwardRef<HTMLDivElement, AlertDialogProps>(function AlertDialog(props, ref) {
  const { open, defaultOpen, onOpenChange, trigger, inline } = props;
  const [inner, setInner] = useState(defaultOpen ?? false);
  const isOpen = open ?? inner;
  const setOpen = (o: boolean) => {
    if (open === undefined) setInner(o);
    onOpenChange?.(o);
  };
  const [busy, setBusy] = useState(false);
  const focus = useRestoreFocus(isOpen);
  const container = usePortalContainer();
  const overSheet = use(OverNonModalSheet);
  const settled = useScopedDialog(inline ? false : isOpen);
  if (inline) return <AlertFrame {...props} close={() => undefined} busy={busy} setBusy={setBusy} rootRef={ref} />;
  return (
    <BaseAlertDialog.Root
      open={isOpen}
      onOpenChange={(o: boolean, details: BaseAlertDialog.Root.ChangeEventDetails) => {
        if (!o && (busy || props.loading)) return details.cancel();
        setOpen(o);
      }}
      onOpenChangeComplete={settled}
    >
      {trigger && <BaseAlertDialog.Trigger render={trigger} />}
      <BaseAlertDialog.Portal container={container}>
        <BaseAlertDialog.Backdrop className={BACKDROP} forceRender={overSheet} />
        <BaseAlertDialog.Viewport className={cn(DIALOG_VIEWPORT, KAESO_DIALOG_VARIANTS.anchor.center.classes)}>
          <BaseAlertDialog.Popup ref={ref} initialFocus={focus.initialFocus} finalFocus={focus.finalFocus} className={cn(DIALOG_POPUP, "w-440")}>
            <InsetContent inset={false}>
              <AlertFrame {...props} close={() => setOpen(false)} busy={busy} setBusy={setBusy} />
            </InsetContent>
          </BaseAlertDialog.Popup>
        </BaseAlertDialog.Viewport>
      </BaseAlertDialog.Portal>
    </BaseAlertDialog.Root>
  );
});
AlertDialog.displayName = "AlertDialog";

export const KAESO_LAYER_DIALOG_VARIANTS = KAESO_DIALOG_VARIANTS;
export const KAESO_LAYER_DIALOG_DEFAULT_VARIANTS = KAESO_DIALOG_DEFAULT_VARIANTS;

export interface DialogLayer {
  title: ReactNode;
  description?: ReactNode | undefined;
  content: ReactNode;
  footer?: ReactNode | undefined;
}

export interface LayerDialogProps extends Omit<DialogProps, "title" | "description" | "children" | "footer" | "headerStart"> {
  layers: DialogLayer[];
  layer?: number | undefined;
  onLayerChange?: ((layer: number) => void) | undefined;
}

const bodyOf = (el: HTMLElement | null | undefined) => el?.closest<HTMLElement>("[data-dialog-body]") ?? null;

export const LayerDialog = forwardRef<HTMLDivElement, LayerDialogProps>(function LayerDialog({ layers, layer, onLayerChange, ...props }, ref) {
  const [inner, setInner] = useState(0);
  const t = useUiText();
  const wanted = layer ?? inner;
  const index = wanted > 0 && wanted < layers.length ? wanted : 0;
  const sections = useRef<(HTMLDivElement | null)[]>([]);
  const focused = useRef<(HTMLElement | null)[]>([]);
  const tops = useRef<number[]>([]);
  const left = useRef(index);
  const shown = useRef(index);
  if (left.current !== index) {
    tops.current[left.current] = bodyOf(sections.current[left.current])?.scrollTop ?? 0;
    left.current = index;
  }
  useLayoutEffect(() => {
    const from = shown.current;
    shown.current = index;
    if (from === index) return;
    const back = index < from;
    if (!back) focused.current[index] = null;
    focused.current.length = index + 1;
    const section = sections.current[index];
    const body = bodyOf(section);
    if (body) body.scrollTop = back ? (tops.current[index] ?? 0) : 0;
    focusDialogSection(section, back ? focused.current[index] : null);
  }, [index]);
  const go = (i: number) => {
    if (props.busy) return;
    if (layer === undefined) setInner(i);
    onLayerChange?.(i);
  };
  const stack = layers.slice(0, index + 1);
  const current = stack[index];
  if (!current) return null;
  const remember = (i: number) => (e: FocusEvent<HTMLElement>) => {
    if (e.target instanceof HTMLElement) focused.current[i] = e.target;
  };
  return (
    <Dialog
      ref={ref}
      {...props}
      title={current.title}
      description={current.description}
      footer={
        current.footer
          ? stack.map((l, i) =>
              l.footer ? (
                <div key={i} hidden={i !== index} className="contents" onFocus={remember(i)}>
                  {l.footer}
                </div>
              ) : null,
            )
          : undefined
      }
      headerStart={
        index > 0 ? (
          <IconButton
            key="back"
            icon={icons.chevronLeft}
            aria-label={t("dialog.back", "Back")}
            onClick={() => go(index - 1)}
            disabled={!!props.busy}
            className="animate-layer-in"
          />
        ) : null
      }
    >
      {stack.map((l, i) => (
        <div
          key={i}
          ref={(el) => {
            sections.current[i] = el;
          }}
          hidden={i !== index}
          className="animate-layer-in"
          onFocus={remember(i)}
        >
          {l.content}
        </div>
      ))}
    </Dialog>
  );
});
LayerDialog.displayName = "LayerDialog";
