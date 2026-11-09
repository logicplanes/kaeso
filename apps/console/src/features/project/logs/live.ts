import { useEffect, useEffectEvent, useMemo, useState, type RefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { LogView, ToolCallView } from "~/api/generated/types";
import { newestLogsQuery, newestToolCallsQuery, type LogFilters, type Newest, type ToolCallFilters } from "~/api/queries/logs";

const POLL_MS = 5000;
const FRESH_MS = 1000;
export const MAX_LIVE_ROWS = 1000;

export function scrollParentOf(el: HTMLElement | null): HTMLElement | null {
  for (let node = el?.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === "auto" || overflow === "scroll") return node;
  }
  return document.scrollingElement as HTMLElement | null;
}

interface Row {
  id: string;
  started_at: string;
}

interface LiveState<T> {
  key: string;
  extra: T[];
  pending: T[];
  pendingCut: boolean;
  cut: boolean;
}

const empty = <T>(key: string): LiveState<T> => ({ key, extra: [], pending: [], pendingCut: false, cut: false });

export interface LiveRows<T> {
  rows: T[];
  pending: T[];
  fresh: ReadonlySet<string>;
  cut: boolean;
  showPending: () => void;
}

const newestStart = (lists: Row[][]): string | undefined =>
  lists.reduce<string | undefined>((newest, list) => {
    const at = list[0]?.started_at;
    return at && (!newest || at > newest) ? at : newest;
  }, undefined);

function addOnTop<T>(list: T[], incoming: T[], connected: boolean, max: number): { list: T[]; cut: boolean } {
  const merged = connected ? [...incoming, ...list] : incoming;
  return { list: merged.slice(0, max), cut: !connected || merged.length > max };
}

interface LiveOptions<T> {
  live: boolean;
  base: T[];
  scroller: RefObject<HTMLElement | null>;
  max?: number;
}

export function useLiveRows<T extends Row>({
  key,
  newest,
  live,
  base,
  scroller,
  max = MAX_LIVE_ROWS,
}: LiveOptions<T> & { key: string; newest: (since: string | undefined) => Promise<Newest<T>> }): LiveRows<T> {
  const [state, setState] = useState<LiveState<T>>(() => empty(""));
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const current = state.key === key ? state : empty<T>(key);
  const { extra, pending, cut } = current;
  const none: T[] = [];
  const shown = useEffectEvent(() => ({
    ids: new Set([...extra, ...pending, ...(cut ? none : base)].map((r) => r.id)),
    since: newestStart([pending, extra, cut ? none : base]),
  }));
  const fetchNewest = useEffectEvent((since: string | undefined) => newest(since));

  useEffect(() => {
    if (!live) return;
    let polling = true;
    const tick = async () => {
      if (document.hidden) return;
      const { since } = shown();
      const newest = await fetchNewest(since).catch((): Newest<T> => ({ rows: [], complete: true }));
      if (!polling) return;
      const { ids } = shown();
      const incoming = newest.rows.filter((r) => !ids.has(r.id));
      if (!incoming.length) return;
      const scrolled = !!scrollParentOf(scroller.current)?.scrollTop;
      setState((s) => {
        const prev = s.key === key ? s : empty<T>(key);
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
  }, [live, key, scroller, max]);

  useEffect(() => {
    if (!fresh.size) return;
    const timer = setTimeout(() => setFresh(new Set()), FRESH_MS);
    return () => clearTimeout(timer);
  }, [fresh]);

  const rows = useMemo(() => {
    const seen = new Set<string>();
    return [...extra, ...(cut ? [] : base)].filter((r) => (seen.has(r.id) ? false : (seen.add(r.id), true)));
  }, [extra, base, cut]);

  const showPending = () => {
    const next = addOnTop(extra, pending, !current.pendingCut, max);
    setState({ key, extra: next.list, pending: [], pendingCut: false, cut: cut || next.cut });
    setFresh(new Set(pending.map((r) => r.id)));
    scrollParentOf(scroller.current)?.scrollTo({ top: 0 });
  };

  return { rows, pending, fresh, cut, showPending };
}

export function useLiveLogs({
  org,
  project,
  filters,
  ...options
}: LiveOptions<LogView> & { org: string; project: string; filters: LogFilters }): LiveRows<LogView> {
  const queryClient = useQueryClient();
  return useLiveRows({
    ...options,
    key: JSON.stringify(["models", org, project, filters]),
    newest: (since) => queryClient.fetchQuery({ ...newestLogsQuery(org, project, filters, since), staleTime: 0 }),
  });
}

export function useLiveToolCalls({
  org,
  project,
  filters,
  ...options
}: LiveOptions<ToolCallView> & { org: string; project: string; filters: ToolCallFilters }): LiveRows<ToolCallView> {
  const queryClient = useQueryClient();
  return useLiveRows({
    ...options,
    key: JSON.stringify(["tools", org, project, filters]),
    newest: (since) => queryClient.fetchQuery({ ...newestToolCallsQuery(org, project, filters, since), staleTime: 0 }),
  });
}
