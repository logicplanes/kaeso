import { useCallback, useEffect, useRef, useState } from "react";
import { isApiError } from "~/api/errors";
import type { RouteStep } from "~/api/generated/types";
import { translateMessage } from "~/api/gateway-errors";
import { streamPlayground, type PlaygroundRequest } from "~/api/playground";
import { t } from "~/i18n";
import { applyEvent, endResponse, isLive, newResponse, type ResponseState } from "../../playground/responses";
import { walk, type DraftCondition, type DraftSplit, type DraftStep } from "./draft";

export type Walked = { path: string; model: string | null; status: number | null; ms: number | null };

export const ROUTE_HEADER = "kaeso-route";

function sfString(text: string, at: number): { value: string; end: number } | null {
  if (text[at] !== '"') return null;
  let value = "";
  for (let i = at + 1; i < text.length; i++) {
    const c = text[i];
    if (c === "\\") {
      value += text[i + 1] ?? "";
      i++;
    } else if (c === '"') return { value, end: i + 1 };
    else value += c;
  }
  return null;
}

export function parseRoute(header: string | null): Walked[] {
  if (!header) return [];
  const out: Walked[] = [];
  let i = 0;
  while (i < header.length) {
    while (header[i] === " " || header[i] === ",") i++;
    const path = sfString(header, i);
    if (!path) break;
    i = path.end;
    const step: Walked = { path: path.value, model: null, status: null, ms: null };
    while (header[i] === ";") {
      const key = /^[a-z*][a-z0-9_.*-]*/.exec(header.slice(i + 1))?.[0] ?? "";
      i += 1 + key.length;
      if (header[i] !== "=") continue;
      i++;
      const quoted = sfString(header, i);
      if (quoted) {
        i = quoted.end;
        if (key === "model") step.model = quoted.value;
        continue;
      }
      const number = /^-?\d+/.exec(header.slice(i))?.[0] ?? "";
      i += number.length;
      if (key === "status") step.status = Number(number);
      if (key === "dur") step.ms = Number(number);
    }
    out.push(step);
  }
  return out;
}

export type Tone = "success" | "warning" | "danger";
export const toneOf = (status: number): Tone => (status >= 500 || status < 200 ? "danger" : status >= 400 ? "warning" : "success");

export type Passed =
  | { kind: "decision"; step: DraftCondition | DraftSplit; path: string; taken: string }
  | { kind: "tried"; step: DraftStep; path: string; walked: Walked; last: boolean };

export function passed(root: DraftStep, walked: readonly Walked[]): Passed[] {
  const byPath = new Map([...walk(root)].map(({ step, path }) => [path, step]));
  const listed = new Set<string>();
  const out: Passed[] = [];
  walked.forEach((w, index) => {
    const parts = w.path.split(".");
    for (let end = 1; end < parts.length; end++) {
      const path = parts.slice(0, end).join(".");
      const step = byPath.get(path);
      if (!step || step.type === "model" || listed.has(path)) continue;
      listed.add(path);
      out.push({ kind: "decision", step, path, taken: parts.slice(end).join(".") });
    }
    const step = byPath.get(w.path);
    if (step) out.push({ kind: "tried", step, path: w.path, walked: w, last: index === walked.length - 1 });
  });
  return out;
}

export function pathIds(root: DraftStep, walked: readonly Walked[]): string[] {
  return passed(root, walked).map((p) => p.step.id);
}

export type Sent = {
  prompt: string;
  route: RouteStep;
  headers: { name: string; value: string }[];
  response: ResponseState;
  walked: Walked[] | null;
  status: number | null;
  requestId: string | null;
};

export function useRouteSend(org: string, project: string) {
  const [sent, setSent] = useState<Sent | null>(null);
  const controller = useRef<AbortController | null>(null);
  const live = useRef<Sent | null>(null);
  const frame = useRef(0);

  // requestAnimationFrame ids start at 1, so 0 is none and cancelling it does nothing.
  const flush = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = 0;
    setSent(live.current);
  }, []);

  useEffect(
    () => () => {
      controller.current?.abort();
      cancelAnimationFrame(frame.current);
    },
    [],
  );

  const send = useCallback(
    async (prompt: string, request: PlaygroundRequest & { route: RouteStep }) => {
      controller.current?.abort();
      const abort = new AbortController();
      controller.current = abort;
      let current: Sent = {
        prompt,
        route: request.route,
        headers: request.headers ?? [],
        response: newResponse(performance.now()),
        walked: null,
        status: null,
        requestId: null,
      };
      const publish = (change: (s: Sent) => Sent, now: boolean) => {
        current = change(current);
        live.current = current;
        if (now) flush();
        else if (!frame.current) frame.current = requestAnimationFrame(flush);
      };
      publish((s) => s, true);
      const answered = (answer: Response) =>
        publish(
          (s) => ({ ...s, walked: parseRoute(answer.headers.get(ROUTE_HEADER)), status: answer.status, requestId: answer.headers.get("kaeso-request-id") }),
          true,
        );
      let failure: unknown = null;
      try {
        for await (const event of streamPlayground(org, project, request, abort.signal, answered)) {
          let data: unknown;
          try {
            data = JSON.parse(event.data);
          } catch {
            continue;
          }
          if (!data || typeof data !== "object") continue;
          const state = current.response;
          const next = applyEvent(state, event.event, data as Record<string, unknown>, performance.now());
          if (next !== state) publish((s) => ({ ...s, response: next }), !isLive(next));
        }
      } catch (e) {
        failure = e;
      }
      const state = current.response;
      if (!isLive(state)) return;
      const now = performance.now();
      const ended = abort.signal.aborted
        ? endResponse(state, "stopped", now)
        : failure === null
          ? endResponse(state, "failed", now, t("playground.streamEnded", "The stream ended before the model finished. Try again."))
          : endResponse(
              state,
              "failed",
              now,
              isApiError(failure)
                ? translateMessage(failure.message)
                : t("playground.connectionClosed", "The connection closed before the reply finished. Try again."),
            );
      publish((s) => ({ ...s, response: ended }), true);
    },
    [org, project, flush],
  );

  const stop = useCallback(() => controller.current?.abort(), []);

  return { sent, send, stop, busy: !!sent && isLive(sent.response) };
}
