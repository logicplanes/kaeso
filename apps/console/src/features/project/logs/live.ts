import { useEffect, useEffectEvent, useMemo, useState, type RefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { LogView } from "~/api/generated/types";
import { newestLogsQuery, type LogFilters, type NewestLogs } from "~/api/queries/logs";

const POLL_MS = 5000;
const FRESH_MS = 1000;
export const MAX_LIVE_ROWS = 1000;
const NONE: LogView[] = [];

export function scrollParentOf(el: HTMLElement | null): HTMLElement | null {
  for (let node = el?.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") return node;
  }
  return document.scrollingElement as HTMLElement | null;
}

interface LiveState {
  key: string;
  extra: LogView[];
  pending: LogView[];
  pendingCut: boolean;
  cut: boolean;
}

const EMPTY: Omit<LiveState, "key"> = { extra: NONE, pending: NONE, pendingCut: false, cut: false };

export interface LiveLogs {
  rows: LogView[];
  pending: LogView[];
  fresh: ReadonlySet<string>;
  cut: boolean;
  showPending: () => void;
}

const newestStart = (lists: LogView[][]): string | undefined =>
  lists.reduce<string | undefined>((newest, list) => {
    const at = list[0]?.started_at;
    return at && (!newest || at > newest) ? at : newest;
  }, undefined);

function addOnTop(list: LogView[], incoming: LogView[], connected: boolean, max: number): { list: LogView[]; cut: boolean } {
  const merged = connected ? [...incoming, ...list] : incoming;
  return { list: merged.slice(0, max), cut: !connected || merged.length > max };
}

export function useLiveLogs({
  org,
  project,
  filters,
  live,
  base,
  scroller,
  max = MAX_LIVE_ROWS,
}: {
  org: string;
  project: string;
  filters: LogFilters;
  live: boolean;
  base: LogView[];
  scroller: RefObject<HTMLElement | null>;
  max?: number;
}): LiveLogs {
  const queryClient = useQueryClient();
  const filterKey = JSON.stringify(filters);
  const [state, setState] = useState<LiveState>({ key: "", ...EMPTY });
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const current = state.key === filterKey ? state : { key: filterKey, ...EMPTY };
  const { extra, pending, cut } = current;
  const shown = useEffectEvent(() => ({
    ids: new Set([...extra, ...pending, ...(cut ? NONE : base)].map((r) => r.id)),
    since: newestStart([pending, extra, cut ? NONE : base]),
  }));

  useEffect(() => {
    if (!live) return;
    let polling = true;
    const tick = async () => {
      if (document.hidden) return;
      const { since } = shown();
      const newest = await queryClient
        .fetchQuery({ ...newestLogsQuery(org, project, JSON.parse(filterKey) as LogFilters, since), staleTime: 0 })
        .catch((): NewestLogs => ({ rows: [], complete: true }));
      if (!polling) return;
      const { ids } = shown();
      const incoming = newest.rows.filter((r) => !ids.has(r.id));
      if (!incoming.length) return;
      const scrolled = !!scrollParentOf(scroller.current)?.scrollTop;
      setState((s) => {
        const prev = s.key === filterKey ? s : { key: filterKey, ...EMPTY };
        if (scrolled) {
          const next = addOnTop(prev.pending, incoming, newest.complete, max);
          return { ...prev, pending: next.list, pendingCut: prev.pendingCut || next.cut };
        }
        const next = addOnTop(prev.extra, incoming, newest.complete, max);
        return { ...prev, extra: next.list, cut: prev.cut || next.cut };
      });
      if (!scrolled) setFresh(new Set(incoming.map((r) => r.id)));
    };
    const timer = setInterval(() => void tick(), POLL_MS);
    return () => {
      polling = false;
      clearInterval(timer);
    };
  }, [live, org, project, filterKey, queryClient, scroller, max]);

  useEffect(() => {
    if (!fresh.size) return;
    const timer = setTimeout(() => setFresh(new Set()), FRESH_MS);
    return () => clearTimeout(timer);
  }, [fresh]);

  const rows = useMemo(() => {
    const seen = new Set<string>();
    return [...extra, ...(cut ? NONE : base)].filter((r) => (seen.has(r.id) ? false : (seen.add(r.id), true)));
  }, [extra, base, cut]);

  const showPending = () => {
    const next = addOnTop(extra, pending, !current.pendingCut, max);
    setState({ key: filterKey, extra: next.list, pending: NONE, pendingCut: false, cut: cut || next.cut });
    setFresh(new Set(pending.map((r) => r.id)));
    scrollParentOf(scroller.current)?.scrollTo({ top: 0 });
  };

  return { rows, pending, fresh, cut, showPending };
}
