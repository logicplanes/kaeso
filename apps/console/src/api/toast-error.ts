import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import type { ApiError } from "./errors";

export function toastApiError(error: ApiError, options: { action?: { label: string; onClick: () => void }; id?: string } = {}): void {
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
