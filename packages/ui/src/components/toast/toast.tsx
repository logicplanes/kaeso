import { forwardRef, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Toast as BaseToast } from "@base-ui/react/toast";
import { usePortalContainer } from "../../utils/portal-context.js";
import { observeResize } from "../../utils/observe-resize.js";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { Spinner } from "../loader/loader.js";
import { SwapText } from "../swap-text/swap-text.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TOAST_VARIANTS = {
  tone: {
    neutral: { classes: "", description: "A plain message (default)" },
    success: { classes: "text-success", description: "Success: a green check before the words" },
    error: { classes: "text-danger", description: "Errors: a red mark; announced as an alert" },
    warning: { classes: "text-warning", description: "Warnings" },
    info: { classes: "text-info", description: "Information" },
    loading: { classes: "text-toast-fg-secondary", description: "Work underway: a spinner" },
  },
} as const;
export const KAESO_TOAST_DEFAULT_VARIANTS = { tone: "neutral" } as const;
type Tone = keyof typeof KAESO_TOAST_VARIANTS.tone;

const ICON = { success: icons.success, error: icons.error, warning: icons.warning, info: icons.info } as const;

export const toastManager = BaseToast.createToastManager();

export interface ToastOptions {
  description?: ReactNode | undefined;
  action?: { label: string; onClick: () => void } | undefined;
  timeout?: number | undefined;
  id?: string | undefined;
}

interface ToastData {
  reserve?: ReactNode[] | undefined;
}

function add(tone: Tone, title: ReactNode, o: ToastOptions = {}) {
  return toastManager.add({
    title,
    type: tone,
    ...(o.description !== undefined ? { description: o.description } : {}),
    ...(o.timeout !== undefined ? { timeout: o.timeout } : {}),
    ...(o.id ? { id: o.id } : {}),
    ...(tone === "error" ? { priority: "high" as const } : {}),
    ...(o.action ? { actionProps: { children: o.action.label, onClick: o.action.onClick } } : {}),
  });
}

export const toast = Object.assign((title: ReactNode, o?: ToastOptions) => add("neutral", title, o), {
  success: (title: ReactNode, o?: ToastOptions) => add("success", title, o),
  error: (title: ReactNode, o?: ToastOptions) => add("error", title, o),
  warning: (title: ReactNode, o?: ToastOptions) => add("warning", title, o),
  info: (title: ReactNode, o?: ToastOptions) => add("info", title, o),
  dismiss: (id?: string) => toastManager.close(id),
  promise: <T,>(p: Promise<T>, m: { loading: string; success: string | ((v: T) => string); error: string | ((e: unknown) => string) }) => {
    const reserve = [m.loading, ...(typeof m.success === "string" ? [m.success] : []), ...(typeof m.error === "string" ? [m.error] : [])];
    return toastManager.promise(p, {
      loading: { title: m.loading, type: "loading", timeout: 0, data: { reserve } },
      success: (v: T) => ({ title: typeof m.success === "function" ? m.success(v) : m.success, type: "success", data: { reserve } }),
      error: (e: unknown) => ({ title: typeof m.error === "function" ? m.error(e) : m.error, type: "error", priority: "high", data: { reserve } }),
    });
  },
});

function ToneIcon({ tone }: { tone: Tone }) {
  if (tone === "neutral") return null;
  return (
    <span className={cn("flex shrink-0", KAESO_TOAST_VARIANTS.tone[tone].classes)}>
      {tone === "loading" ? <Spinner size="sm" /> : <Icon icon={ICON[tone]} size="sm" />}
    </span>
  );
}

const SURFACE = "relative flex min-h-44 max-w-560 items-center gap-10 bg-toast py-8 text-body font-medium text-toast-fg shadow-toast";
const ACTION =
  "inline-flex h-32 shrink-0 cursor-pointer items-center rounded-pill bg-toast-action px-12 text-body font-medium text-toast-fg transition-control hover:bg-toast-action-hover focus-ring";
const CLOSE =
  "k-toast-close absolute -top-6 -left-6 inline-flex size-20 cursor-pointer items-center justify-center rounded-pill bg-toast text-toast-fg-secondary shadow-toast hover:text-toast-fg focus-ring";

function useMultiline(title: ReactNode, description: ReactNode) {
  const ref = useRef<HTMLDivElement>(null);
  const [multi, setMulti] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el) setMulti(el.offsetHeight > 46);
  }, [title, description]);
  return [ref, multi] as const;
}

export interface ToastCapsuleProps {
  tone?: Tone | undefined;
  title: ReactNode;
  description?: ReactNode | undefined;
  action?: string | undefined;
  closable?: boolean | undefined;
  "data-force"?: string | undefined;
}

export const ToastCapsule = forwardRef<HTMLDivElement, ToastCapsuleProps>(function ToastCapsule(
  { tone = "neutral", title, description, action, closable, "data-force": force },
  ref,
) {
  const [surface, multi] = useMultiline(title, description);
  const t = useUiText();
  return (
    <div ref={ref} role="status" className="k-toast-static relative w-max">
      <div ref={surface} className={cn(SURFACE, multi ? "rounded-toast" : "rounded-pill", tone === "neutral" ? "pl-16" : "pl-14", action ? "pr-6" : "pr-16")}>
        <ToneIcon tone={tone} />
        <span className="min-w-0 text-pretty">
          {title}
          {description && <span className="font-regular text-toast-fg-secondary"> {description}</span>}
        </span>
        {action && (
          <button type="button" className={ACTION}>
            {action}
          </button>
        )}
      </div>
      {closable && (
        <button type="button" aria-label={t("toast.dismiss", "Dismiss")} className={CLOSE} data-force={force ? `visible ${force}` : "visible"}>
          <Icon icon={icons.close} size="xs" className="size-12" />
        </button>
      )}
    </div>
  );
});
ToastCapsule.displayName = "ToastCapsule";

type ManagedToast = ReturnType<typeof BaseToast.useToastManager>["toasts"][number];

function ToastItem({ item, index, limit, onFrontWidth }: { item: ManagedToast; index: number; limit: number; onFrontWidth: (w: number) => void }) {
  const t = useUiText();
  const tone = ((item.type as Tone) ?? "neutral") as Tone;
  const reserve = (item.data as ToastData | undefined)?.reserve;
  const [ref, multi] = useMultiline(item.title, item.description);
  useLayoutEffect(() => {
    const el = ref.current;
    if (index !== 0 || !el) return;
    const report = () => onFrontWidth(Math.round(el.offsetWidth));
    report();
    return observeResize([el], report);
  }, [index, onFrontWidth, ref]);
  return (
    <BaseToast.Root
      toast={item}
      swipeDirection={["down", "left", "right"]}
      className="k-toast outline-none"
      {...(index > 0 ? { "data-behind": "" } : {})}
      {...(index >= limit ? { "data-index-hidden": "" } : {})}
    >
      <div
        ref={ref}
        className={cn(
          "k-toast-surface",
          SURFACE,
          multi ? "rounded-toast" : "rounded-pill",
          tone === "neutral" ? "pl-16" : "pl-14",
          item.actionProps ? "pr-6" : "pr-16",
        )}
      >
        <div className="k-toast-content flex min-w-0 items-center gap-10">
          <ToneIcon tone={tone} />
          <span className="min-w-0 text-pretty">
            {reserve ? (
              <SwapText reserve={reserve} align="start">
                <BaseToast.Title render={<span />} />
              </SwapText>
            ) : (
              <BaseToast.Title render={<span />} />
            )}
            {item.description && (
              <>
                {" "}
                <BaseToast.Description render={<span />} className="font-regular text-toast-fg-secondary" />
              </>
            )}
          </span>
          {item.actionProps && <BaseToast.Action className={ACTION} />}
        </div>
      </div>
      <BaseToast.Close aria-label={t("toast.dismiss", "Dismiss")} className={CLOSE}>
        <Icon icon={icons.close} size="xs" className="size-12" />
      </BaseToast.Close>
    </BaseToast.Root>
  );
}

function List({ limit }: { limit: number }) {
  const { toasts } = BaseToast.useToastManager();
  const live = toasts.filter((item) => item.transitionStatus !== "ending");
  const [frontWidth, setFrontWidth] = useState(0);
  const t = useUiText();
  return (
    <BaseToast.Portal container={usePortalContainer()}>
      <BaseToast.Viewport
        aria-label={t("toast.notifications", "Notifications")}
        className="fixed right-0 bottom-24 left-0 z-60 mx-auto h-0 w-full toast-viewport-width outline-none"
        style={{ ["--k-front-width" as string]: frontWidth ? `${frontWidth}px` : "auto" }}
      >
        {toasts.map((item) => (
          <ToastItem key={item.id} item={item} index={Math.max(0, live.indexOf(item))} limit={limit} onFrontWidth={setFrontWidth} />
        ))}
      </BaseToast.Viewport>
    </BaseToast.Portal>
  );
}

export function Toaster({ limit = 3, timeout = 5000 }: { limit?: number | undefined; timeout?: number | undefined }) {
  return (
    <BaseToast.Provider toastManager={toastManager} limit={limit} timeout={timeout}>
      <List limit={limit} />
    </BaseToast.Provider>
  );
}
Toaster.displayName = "Toaster";

export const Toast = Object.assign(
  function Toast(props: { limit?: number | undefined; timeout?: number | undefined }) {
    return <Toaster {...props} />;
  },
  { add: toast },
);
