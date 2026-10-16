import type { ReactNode } from "react";
import type { ToastOptions } from "@logicplanes/kaeso-ui";

let toasterMounted: () => void;
let isMounted = false;
let mounted = new Promise<void>((resolve) => (toasterMounted = resolve));

export const markToasterMounted = () => {
  isMounted = true;
  toasterMounted();
  return () => {
    isMounted = false;
    mounted = new Promise<void>((resolve) => (toasterMounted = resolve));
  };
};

export const toasterIsMounted = () => isMounted;

const toastModule = () => Promise.all([import("@logicplanes/kaeso-ui/toast"), mounted]).then(([m]) => m);
const load = () => toastModule().then((m) => m.toast);

type Tone = "success" | "error" | "warning";
const show = (tone: Tone | null, title: ReactNode, options?: ToastOptions) => void load().then((t) => (tone ? t[tone](title, options) : t(title, options)));

export const toast = Object.assign((title: ReactNode, options?: ToastOptions) => show(null, title, options), {
  success: (title: ReactNode, options?: ToastOptions) => show("success", title, options),
  error: (title: ReactNode, options?: ToastOptions) => show("error", title, options),
  warning: (title: ReactNode, options?: ToastOptions) => show("warning", title, options),
  dismiss: (id?: string) => void load().then((t) => t.dismiss(id)),
});

export function progressToast(id: string, title: ReactNode, options: { reserve: ReactNode[]; action: { label: string; onClick: () => void } }): void {
  void toastModule().then((m) =>
    m.toastManager.add({
      id,
      title,
      type: "loading",
      timeout: 0,
      data: { reserve: options.reserve },
      actionProps: { children: options.action.label, onClick: options.action.onClick },
    }),
  );
}

export const updateProgressToast = (id: string, title: ReactNode) => void toastModule().then((m) => m.toastManager.update(id, { title }));

export const closeToast = (id: string) => void toastModule().then((m) => m.toastManager.close(id));
