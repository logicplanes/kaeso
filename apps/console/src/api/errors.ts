import { locale, t } from "~/i18n";
import type { components } from "./generated/schema";

export type ErrorType = components["schemas"]["ErrorBody"]["type"];

export class ApiError extends Error {
  readonly type: ErrorType | "network";
  readonly status: number;
  readonly param: string | null;
  readonly requestId: string | null;
  readonly right: string | null;

  constructor(init: { type: ErrorType | "network"; status: number; message: string; param?: string | null; requestId?: string | null; right?: string | null }) {
    super(init.message);
    this.name = "ApiError";
    this.type = init.type;
    this.status = init.status;
    this.param = init.param ?? null;
    this.requestId = init.requestId ?? null;
    this.right = init.right ?? null;
  }

  get unreachable(): boolean {
    return this.type === "network" || this.type === "unavailable";
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

export async function errorFromResponse(response: Response): Promise<ApiError> {
  const requestId = response.headers.get("kaeso-request-id");
  let body: unknown;
  try {
    body = await response.clone().json();
  } catch {
    body = null;
  }
  const error = (body as { error?: { type?: string; message?: string; param?: string | null; right?: string | null } } | null)?.error;
  if (error?.type && error.message) {
    return new ApiError({
      type: error.type as ErrorType,
      status: response.status,
      message: await translated(error.message),
      param: error.param ?? null,
      requestId,
      right: error.right ?? null,
    });
  }
  return new ApiError({
    type: response.status >= 500 ? "internal" : "invalid_request",
    status: response.status,
    message:
      response.status === 502 || response.status === 504
        ? t("common.unreachable", "Can't reach the gateway. Try again.")
        : t("errors.gatewayAnswered", "The gateway answered {status}.", { status: String(response.status) }),
    requestId,
  });
}

export const networkError = () => new ApiError({ type: "network", status: 0, message: t("common.unreachable", "Can't reach the gateway. Try again.") });

async function translated(message: string): Promise<string> {
  if (locale() === "en") return message;
  try {
    return (await import("./gateway-errors")).translateMessage(message);
  } catch {
    return message;
  }
}
