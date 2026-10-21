import { useCallback, useEffect, useRef, useState } from "react";
import { isApiError } from "~/api/errors";
import { streamPlayground, type PlaygroundRequest } from "~/api/playground";
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
}

const settled = (threads: Thread[], now: number): Thread[] =>
  threads.map((t) => ({ ...t, turns: t.turns.map((x) => (isLive(x.response) ? { ...x, response: endResponse(x.response, "stopped", now) } : x)) }));

export function useThreads(
  org: string,
  project: string,
  columns: number,
  { initial, onLeave }: { initial?: Thread[] | undefined; onLeave?: ((threads: Thread[]) => void) | undefined } = {},
) {
  const [threads, setThreads] = useState<Thread[]>(() => (initial?.length === columns ? initial : Array.from({ length: columns }, newThread)));
  if (threads.length !== columns) {
    setThreads(threads.length > columns ? threads.slice(0, columns) : [...threads, ...Array.from({ length: columns - threads.length }, newThread)]);
  }

  const controllers = useRef(new Map<string, AbortController>());
  const finished = useRef(new Set<string>());
  const live = useRef(new Map<string, ResponseState>());
  const frame = useRef<number | null>(null);
  const mounted = useRef(true);
  const latest = useRef(threads);
  const leave = useRef(onLeave);
  useEffect(() => {
    latest.current = threads;
    leave.current = onLeave;
  });

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
      leave.current?.(settled(latest.current, performance.now()));
      for (const c of open.values()) c.abort();
      open.clear();
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    };
  }, []);

  const settle = useCallback(
    async (turn: Turn, state: ResponseState) => {
      const settled = state.status === "failed" && state.error ? { ...state, error: await inLanguage(state.error) } : state;
      live.current.set(turn.id, settled);
      if (mounted.current) flush();
    },
    [flush],
  );

  const run = useCallback(
    async (turn: Turn, request: PlaygroundRequest) => {
      const controller = new AbortController();
      controllers.current.set(turn.id, controller);
      let state = turn.response;
      let settled = false;
      try {
        for await (const event of streamPlayground(org, project, request, controller.signal)) {
          let data: Record<string, unknown>;
          try {
            data = JSON.parse(event.data) as Record<string, unknown>;
          } catch {
            continue;
          }
          if (!data || typeof data !== "object") continue;
          const next = applyEvent(state, event.event, data, performance.now());
          if (next === state) continue;
          state = next;
          if (isLive(state)) {
            live.current.set(turn.id, state);
            schedule();
            continue;
          }
          settled = true;
          finished.current.add(turn.id);
          void settle(turn, state);
        }
        if (isLive(state))
          state = controller.signal.aborted
            ? endResponse(state, "stopped", performance.now())
            : endResponse(state, "failed", performance.now(), t("playground.streamEnded", "The stream ended before the model finished. Try again."));
      } catch (e) {
        if (controller.signal.aborted) state = endResponse(state, "stopped", performance.now());
        else
          state = endResponse(
            state,
            "failed",
            performance.now(),
            isApiError(e) ? e.message : t("playground.connectionClosed", "The connection closed before the reply finished. Try again."),
            isApiError(e) && e.status >= 500 ? e.requestId : null,
          );
      } finally {
        controllers.current.delete(turn.id);
        finished.current.delete(turn.id);
      }
      if (!settled) await settle(turn, state);
    },
    [org, project, schedule, settle],
  );

  const send = useCallback(
    (prompt: string, sends: Send[]) => {
      const now = performance.now();
      const turns = sends.map((s) => ({ ...s, turn: { id: id("turn"), prompt, response: newResponse(now) } satisfies Turn }));
      for (const t of turns) live.current.set(t.turn.id, t.turn.response);
      setThreads((ts) => ts.map((t, i) => ({ ...t, turns: [...t.turns, ...turns.filter((x) => x.column === i).map((x) => x.turn)] })));
      for (const t of turns) void run(t.turn, t.request);
    },
    [run],
  );

  const retry = useCallback(
    (column: number, request: PlaygroundRequest) => {
      const thread = threads[column];
      const last = thread?.turns.at(-1);
      if (!thread || !last || isLive(last.response)) return;
      const turn: Turn = { id: id("turn"), prompt: last.prompt, response: newResponse(performance.now()) };
      live.current.set(turn.id, turn.response);
      setThreads((ts) => ts.map((t) => (t.key === thread.key ? { ...t, turns: [...t.turns.slice(0, -1), turn] } : t)));
      void run(turn, request);
    },
    [threads, run],
  );

  const stop = useCallback(() => {
    for (const [turn, c] of controllers.current) if (!finished.current.has(turn)) c.abort();
  }, []);

  const add = useCallback(() => setThreads((ts) => [...ts, newThread()]), []);

  const remove = (column: number) => {
    threads[column]?.turns.forEach((x) => controllers.current.get(x.id)?.abort());
    setThreads((ts) => ts.filter((_, i) => i !== column));
  };

  const clear = (): Thread[] => {
    stop();
    const before = settled(threads, performance.now());
    setThreads((ts) => ts.map((t) => ({ ...t, turns: [] })));
    return before;
  };

  const restore = (before: Thread[]) =>
    setThreads((ts) =>
      ts.map((t) => {
        const kept = before.find((b) => b.key === t.key);
        return kept && t.turns.length === 0 ? kept : t;
      }),
    );

  const busy = threads.some((t) => t.turns.some((x) => isLive(x.response)));
  return { threads, busy, send, retry, stop, add, remove, clear, restore };
}
