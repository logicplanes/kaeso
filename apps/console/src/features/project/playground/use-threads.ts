import { useCallback, useEffect, useRef, useState } from "react";
import { isApiError } from "~/api/errors";
import { streamPlayground, type PlaygroundRequest } from "~/api/playground";
import { toast } from "~/shell/toast";
import { locale, t } from "~/i18n";
import { applyEvent, endResponse, isLive, newResponse, type ResponseState, type Turn } from "./responses";

async function inLanguage(message: string): Promise<string> {
  if (locale() === "en") return message;
  try {
    return (await import("~/api/gateway-errors")).translateMessage(message);
  } catch {
    return message;
  }
}

export interface Thread {
  key: string;
  turns: Turn[];
}

let next = 0;
const id = (prefix: string) => `${prefix}${++next}`;
const newThread = (): Thread => ({ key: id("col"), turns: [] });

export interface Send {
  column: number;
  request: PlaygroundRequest;
  label: string;
}

export function useThreads(org: string, project: string, columns: number) {
  const [threads, setThreads] = useState<Thread[]>(() => Array.from({ length: columns }, newThread));
  if (threads.length !== columns) {
    setThreads(threads.length > columns ? threads.slice(0, columns) : [...threads, ...Array.from({ length: columns - threads.length }, newThread)]);
  }

  const controllers = useRef(new Map<string, AbortController>());
  const live = useRef(new Map<string, ResponseState>());
  const frame = useRef<number | null>(null);
  const mounted = useRef(true);

  const flush = useCallback(() => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    const snapshot = new Map(live.current);
    for (const [turn, r] of snapshot) if (!isLive(r)) live.current.delete(turn);
    setThreads((ts) =>
      ts.map((t) =>
        t.turns.some((x) => snapshot.has(x.id))
          ? { ...t, turns: t.turns.map((x) => (snapshot.has(x.id) ? { ...x, response: snapshot.get(x.id) as ResponseState } : x)) }
          : t,
      ),
    );
  }, []);
  const schedule = useCallback(() => {
    if (frame.current === null) frame.current = requestAnimationFrame(flush);
  }, [flush]);

  useEffect(() => {
    mounted.current = true;
    const open = controllers.current;
    return () => {
      mounted.current = false;
      for (const c of open.values()) c.abort();
      open.clear();
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    };
  }, []);

  const run = useCallback(
    async (turn: Turn, request: PlaygroundRequest, label: string) => {
      const controller = new AbortController();
      controllers.current.set(turn.id, controller);
      let state = turn.response;
      let failure: string | null = null;
      let requestId: string | null = null;
      try {
        for await (const event of streamPlayground(org, project, request, controller.signal)) {
          let data: Record<string, unknown>;
          try {
            data = JSON.parse(event.data) as Record<string, unknown>;
          } catch {
            continue;
          }
          if (!data || typeof data !== "object") continue;
          state = applyEvent(state, event.event, data, performance.now());
          live.current.set(turn.id, state);
          if (!isLive(state)) break;
          schedule();
        }
        if (isLive(state))
          state = controller.signal.aborted
            ? endResponse(state, "stopped", performance.now())
            : endResponse(state, "failed", performance.now(), t("playground.streamEnded", "The stream ended before the model finished. Try again."));
        if (state.status === "failed" && state.error) failure = await inLanguage(state.error);
      } catch (e) {
        if (controller.signal.aborted) state = endResponse(state, "stopped", performance.now());
        else {
          failure = isApiError(e) ? e.message : t("playground.connectionClosed", "The connection closed before the reply finished. Try again.");
          requestId = isApiError(e) && e.status >= 500 ? e.requestId : null;
          state = endResponse(state, "failed", performance.now(), failure);
        }
      } finally {
        controllers.current.delete(turn.id);
        live.current.set(turn.id, state);
        flush();
      }
      if (failure && mounted.current)
        toast.error(failure, {
          id: `playground:${failure}`,
          description: requestId ? t("playground.failedRequest", "{model} · Request {id}", { model: label, id: requestId }) : label,
        });
    },
    [org, project, schedule, flush],
  );

  const send = useCallback(
    (prompt: string, sends: Send[]) => {
      const now = performance.now();
      const turns = sends.map((s) => ({ ...s, turn: { id: id("turn"), prompt, response: newResponse(now) } satisfies Turn }));
      for (const t of turns) live.current.set(t.turn.id, t.turn.response);
      setThreads((ts) => ts.map((t, i) => ({ ...t, turns: [...t.turns, ...turns.filter((x) => x.column === i).map((x) => x.turn)] })));
      for (const t of turns) void run(t.turn, t.request, t.label);
    },
    [run],
  );

  const retry = useCallback(
    (column: number, request: PlaygroundRequest, label: string) => {
      const thread = threads[column];
      const last = thread?.turns.at(-1);
      if (!thread || !last || isLive(last.response)) return;
      const turn: Turn = { id: id("turn"), prompt: last.prompt, response: newResponse(performance.now()) };
      live.current.set(turn.id, turn.response);
      setThreads((ts) => ts.map((t) => (t.key === thread.key ? { ...t, turns: [...t.turns.slice(0, -1), turn] } : t)));
      void run(turn, request, label);
    },
    [threads, run],
  );

  const stop = useCallback(() => {
    for (const c of controllers.current.values()) c.abort();
  }, []);

  const add = useCallback(() => setThreads((ts) => [...ts, newThread()]), []);

  const remove = (column: number) => {
    threads[column]?.turns.forEach((x) => controllers.current.get(x.id)?.abort());
    setThreads((ts) => ts.filter((_, i) => i !== column));
  };

  const clear = () => {
    stop();
    setThreads((ts) => ts.map((t) => ({ ...t, turns: [] })));
  };

  const busy = threads.some((t) => t.turns.some((x) => isLive(x.response)));
  return { threads, busy, send, retry, stop, add, remove, clear };
}
