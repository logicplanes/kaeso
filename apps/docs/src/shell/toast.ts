import type { ReactNode } from "react";
import type { ToastOptions } from "@logicplanes/kaeso-ui";

let toasterMounted: () => void = () => undefined;
const mounted = new Promise<void>((resolve) => (toasterMounted = resolve));

export const markToasterMounted = () => toasterMounted();

const load = () => Promise.all([import("@logicplanes/kaeso-ui/toast"), mounted]).then(([m]) => m.toast);

export const toast = {
  error: (title: ReactNode, options?: ToastOptions) => void load().then((t) => t.error(title, options)),
};
