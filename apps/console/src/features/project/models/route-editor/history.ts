import { useCallback, useRef, useState } from "react";

export const HISTORY_LIMIT = 100;
const COALESCE_MS = 1000;

type History<T> = { past: T[]; present: T; future: T[] };

export function useHistory<T>(initial: () => T) {
  const [state, setState] = useState<History<T>>(() => ({ past: [], present: initial(), future: [] }));
  const last = useRef<{ key: string; at: number } | null>(null);

  const commit = useCallback((change: (present: T) => T, coalesce?: string) => {
    setState((h) => {
      const next = change(h.present);
      if (Object.is(next, h.present)) return h;
      const now = Date.now();
      const merge = coalesce !== undefined && last.current?.key === coalesce && now - last.current.at < COALESCE_MS;
      last.current = coalesce === undefined ? null : { key: coalesce, at: now };
      if (merge) return { past: h.past, present: next, future: [] };
      return { past: [...h.past, h.present].slice(-HISTORY_LIMIT), present: next, future: [] };
    });
  }, []);

  const undo = useCallback(() => {
    last.current = null;
    setState((h) => {
      const previous = h.past[h.past.length - 1];
      return previous === undefined ? h : { past: h.past.slice(0, -1), present: previous, future: [h.present, ...h.future] };
    });
  }, []);

  const redo = useCallback(() => {
    last.current = null;
    setState((h) => {
      const [next, ...rest] = h.future;
      return next === undefined ? h : { past: [...h.past, h.present], present: next, future: rest };
    });
  }, []);

  return { present: state.present, canUndo: state.past.length > 0, canRedo: state.future.length > 0, commit, undo, redo };
}
