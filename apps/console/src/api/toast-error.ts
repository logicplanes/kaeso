import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import { isApiError } from "./errors";

type ToastErrorOptions = { action?: { label: string; onClick: () => void }; id?: string; description?: string };

export function toastApiError(error: unknown, { fallback, ...options }: ToastErrorOptions & { fallback?: string } = {}): void {
  if (!isApiError(error)) {
    toast.error(fallback ?? t("common.unreachable", "Can't reach the gateway. Try again."), options);
    return;
  }
  if (error.type === "permission_denied" && error.right) {
    void import("~/auth/missing-right-toast").then((m) => m.toastMissingRight(error, options.id ? { id: options.id } : {}));
    return;
  }
  const failed = error.type === "internal" || error.type === "unavailable" || error.type === "network";
  toast.error(error.message, {
    ...(failed && error.requestId ? { description: t("errors.requestId", "Request {id}", { id: error.requestId }) } : {}),
    ...options,
  });
}
