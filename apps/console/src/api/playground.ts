import { t } from "~/i18n";
import { api } from "./client";
import { ApiError } from "./errors";
import type { PlaygroundRequest as Body } from "./generated/types";
import { readSse, type SseEvent } from "./sse";

export type PlaygroundRequest = Body & { stream: true };

export async function* streamPlayground(org: string, project: string, body: PlaygroundRequest, signal: AbortSignal): AsyncGenerator<SseEvent> {
  const { data, response } = await api.POST("/platform/v1/orgs/{org}/projects/{project}/playground/responses", {
    params: { path: { org, project } },
    body,
    headers: { accept: "text/event-stream" },
    parseAs: "stream",
    signal,
  });
  if (!data || !(response.headers.get("content-type") ?? "").includes("text/event-stream")) {
    await data?.cancel().catch(() => undefined);
    throw new ApiError({
      type: "internal",
      status: response.status,
      message: t("errors.playgroundUnreadable", "The gateway sent an answer the playground can't read. Try again."),
      requestId: response.headers.get("kaeso-request-id"),
    });
  }
  yield* readSse(data, signal);
}
