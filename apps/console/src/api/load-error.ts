import { connectionWatched } from "~/shell/connection-watch";
import { isApiError } from "./errors";

export const toastedWhenLoading = (error: unknown): boolean =>
  isApiError(error) && error.type !== "authentication" && error.type !== "not_found" && !(error.unreachable && connectionWatched());

export const untoldReason = (error: unknown): string | undefined => (isApiError(error) && !toastedWhenLoading(error) ? error.message : undefined);
