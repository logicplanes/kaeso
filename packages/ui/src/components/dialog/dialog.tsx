import { InsetContent } from "../../utils/inset.js";
import { usePortalContainer } from "../../utils/portal-context.js";
import {
  forwardRef,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { AlertDialog as BaseAlertDialog } from "@base-ui/react/alert-dialog";
import { cn } from "../../utils/cn.js";
import { useRestoreFocus } from "../../utils/restore-focus.js";
import { useScopedDialog } from "../../utils/dialog-scope.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Button } from "../button/button.js";
import { Input } from "../input/input.js";
import { focusDialogSection, initialDialogControl } from "./focus.js";
import { useResizeMotion } from "../../utils/resize-motion.js";

export const KAESO_DIALOG_VARIANTS = {
  size: {
    sm: { classes: "w-440", description: "440px: confirmations, short forms" },
    md: { classes: "w-560", description: "560px: forms (default)" },
    lg: { classes: "w-720", description: "720px: two columns, previews" },
  },
} as const;
export const KAESO_DIALOG_DEFAULT_VARIANTS = { size: "md" } as const;

const BACKDROP = "fixed inset-0 z-50 bg-overlay backdrop-motion";
const DIALOG_VIEWPORT = "fixed inset-0 z-50 flex items-center justify-center p-16";
const DIALOG_POPUP = "relative flex dialog-limits flex-col overflow-hidden rounded-dialog bg-raised text-fg shadow-dialog outline-none dialog-motion";

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
  size?: keyof typeof KAESO_DIALOG_VARIANTS.size | undefined;
  children?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  onSubmit?: (() => void) | undefined;
  hideClose?: boolean | undefined;
  headerStart?: ReactNode | undefined;
  className?: string | undefined;
  inline?: boolean | undefined;
}

function DialogFrame({ title, description, children, footer, onSubmit, hideClose, headerStart, size = "md", className, inline }: DialogProps) {
  const { ref, under, check } = useScrolledUnder<HTMLDivElement>();
  const [box, boxRef] = useBox(inline);
  useResizeMotion(box, check);
  useKeepFocus(box, inline);
  const t = useUiText();
  const Title = inline ? "h2" : BaseDialog.Title;
  const closeButton = <IconButton icon={icons.close} aria-label={t("dialog.close", "Close")} tooltip={false} />;
  const close = hideClose ? null : inline ? closeButton : <BaseDialog.Close render={closeButton} />;
  const Description = inline ? "p" : BaseDialog.Description;
  return (
    <div
      ref={boxRef}
      data-dialog-frame=""
      className={cn(
        "flex min-h-0 flex-col",
        inline ? "rounded-dialog bg-raised shadow-dialog" : "flex-1 self-center dialog-limits",
        KAESO_DIALOG_VARIANTS.size[size].classes,
        className,
      )}
      onKeyDown={(e: KeyboardEvent) => {
        if (onSubmit && e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          onSubmit();
        }
      }}
    >
      {headerStart !== undefined ? (
        <>
          <div className="flex h-32 items-center justify-between px-16 pt-16 box-content">
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
          {close && <span className="-mr-8 flex">{close}</span>}
        </div>
      )}
      {children && (
        <div ref={ref} data-dialog-body="" className={cn("min-h-0 flex-1 overflow-y-auto px-24 pt-16 scrollbar-quiet", footer ? "pb-8" : "pb-24")}>
          {children}
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
  );
}

function focusFirstControl(): HTMLElement | true {
  const open = [...document.querySelectorAll<HTMLElement>("[role=dialog], [role=alertdialog]")].pop();
  const frame = open?.querySelector<HTMLElement>("[data-dialog-frame]");
  return (frame && initialDialogControl(frame)) ?? open ?? true;
}

const Root = forwardRef<HTMLDivElement, DialogProps>(function Dialog(props, ref) {
  const { open, defaultOpen, onOpenChange, trigger, size = "md", inline } = props;
  const focus = useRestoreFocus(open, focusFirstControl);
  const container = usePortalContainer();
  const settled = useScopedDialog(open);
  if (inline) return <DialogFrame {...props} />;
  return (
    <BaseDialog.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      onOpenChangeComplete={settled}
    >
      {trigger && <BaseDialog.Trigger render={trigger} />}
      <BaseDialog.Portal container={container}>
        <BaseDialog.Backdrop className={BACKDROP} />
        <BaseDialog.Viewport className={DIALOG_VIEWPORT}>
          <BaseDialog.Popup
            ref={ref}
            initialFocus={focus.initialFocus}
            finalFocus={focus.finalFocus}
            className={cn(DIALOG_POPUP, KAESO_DIALOG_VARIANTS.size[size].classes)}
          >
            <InsetContent inset={false}>
              <DialogFrame {...props} />
            </InsetContent>
          </BaseDialog.Popup>
        </BaseDialog.Viewport>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
});
Root.displayName = "Dialog";

function Close({ children }: { children: ReactElement }) {
  return <BaseDialog.Close render={children} />;
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
  onConfirm: () => void | Promise<void>;
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
}: AlertDialogProps & { close: () => void }) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const t = useUiText();
  const [box, boxRef] = useBox(inline);
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
    <div ref={boxRef} className={cn("flex w-440 flex-col gap-20 p-24", inline ? "rounded-dialog bg-raised shadow-dialog" : "self-center dialog-limits")}>
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
        {inline ? <Button>{cancelLabel}</Button> : <BaseAlertDialog.Close render={<Button>{cancelLabel}</Button>} />}
        <Button variant={tone === "danger" ? "danger" : "primary"} onClick={confirm} disabled={!ok} loading={busy || loading}>
          {confirmLabel}
        </Button>
      </div>
    </div>
  );
}

export function AlertDialog(props: AlertDialogProps) {
  const { open, defaultOpen, onOpenChange, trigger, inline } = props;
  const [inner, setInner] = useState(defaultOpen ?? false);
  const isOpen = open ?? inner;
  const setOpen = (o: boolean) => {
    if (open === undefined) setInner(o);
    onOpenChange?.(o);
  };
  const focus = useRestoreFocus(isOpen);
  const container = usePortalContainer();
  const settled = useScopedDialog(isOpen);
  if (inline) return <AlertFrame {...props} close={() => undefined} />;
  return (
    <BaseAlertDialog.Root open={isOpen} onOpenChange={(o: boolean) => setOpen(o)} onOpenChangeComplete={settled}>
      {trigger && <BaseAlertDialog.Trigger render={trigger} />}
      <BaseAlertDialog.Portal container={container}>
        <BaseAlertDialog.Backdrop className={BACKDROP} />
        <BaseAlertDialog.Viewport className={DIALOG_VIEWPORT}>
          <BaseAlertDialog.Popup initialFocus={focus.initialFocus} finalFocus={focus.finalFocus} className={cn(DIALOG_POPUP, "w-440")}>
            <InsetContent inset={false}>
              <AlertFrame {...props} close={() => setOpen(false)} />
            </InsetContent>
          </BaseAlertDialog.Popup>
        </BaseAlertDialog.Viewport>
      </BaseAlertDialog.Portal>
    </BaseAlertDialog.Root>
  );
}
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

export function LayerDialog({ layers, layer, onLayerChange, ...props }: LayerDialogProps) {
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
          <IconButton key="back" icon={icons.chevronLeft} aria-label={t("dialog.back", "Back")} onClick={() => go(index - 1)} className="animate-layer-in" />
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
}
LayerDialog.displayName = "LayerDialog";
