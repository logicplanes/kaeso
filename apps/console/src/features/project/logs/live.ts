import { useEffect, useEffectEvent, useMemo, useState, type RefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { LogView } from "~/api/generated/types";
import { newestLogsQuery, type LogFilters } from "~/api/queries/logs";

const POLL_MS = 5000;
const FRESH_MS = 1000;
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
}

export interface LiveLogs {
  rows: LogView[];
  pending: LogView[];
  fresh: ReadonlySet<string>;
  showPending: () => void;
}

export function useLiveLogs({
  org,
  project,
  filters,
  live,
  base,
  scroller,
}: {
  org: string;
  project: string;
  filters: LogFilters;
  live: boolean;
  base: LogView[];
  scroller: RefObject<HTMLElement | null>;
}): LiveLogs {
  const queryClient = useQueryClient();
  const filterKey = JSON.stringify(filters);
  const [state, setState] = useState<LiveState>({ key: "", extra: [], pending: [] });
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const { extra, pending } = state.key === filterKey ? state : { extra: NONE, pending: NONE };
  const shownIds = useEffectEvent(() => new Set([...extra, ...pending, ...base].map((r) => r.id)));

  useEffect(() => {
    if (!live) return;
    let polling = true;
    const tick = async () => {
      const newest = await queryClient.fetchQuery({ ...newestLogsQuery(org, project, JSON.parse(filterKey) as LogFilters), staleTime: 0 }).catch(() => []);
      if (!polling) return;
      const shown = shownIds();
      const incoming = newest.filter((r) => !shown.has(r.id));
      if (!incoming.length) return;
      const scrolled = !!scrollParentOf(scroller.current)?.scrollTop;
      setState((s) => {
        const prev = s.key === filterKey ? s : { key: filterKey, extra: [], pending: [] };
        return scrolled ? { ...prev, pending: [...incoming, ...prev.pending] } : { ...prev, extra: [...incoming, ...prev.extra] };
      });
      if (!scrolled) setFresh(new Set(incoming.map((r) => r.id)));
    };
    const timer = setInterval(() => void tick(), POLL_MS);
    return () => {
      polling = false;
      clearInterval(timer);
    };
  }, [live, org, project, filterKey, queryClient, scroller]);

  useEffect(() => {
    if (!fresh.size) return;
    const timer = setTimeout(() => setFresh(new Set()), FRESH_MS);
    return () => clearTimeout(timer);
  }, [fresh]);

  const rows = useMemo(() => {
    const seen = new Set<string>();
    return [...extra, ...base].filter((r) => (seen.has(r.id) ? false : (seen.add(r.id), true)));
  }, [extra, base]);

  const showPending = () => {
    setState({ key: filterKey, extra: [...pending, ...extra], pending: [] });
    setFresh(new Set(pending.map((r) => r.id)));
    scrollParentOf(scroller.current)?.scrollTo({ top: 0 });
  };

  return { rows, pending, fresh, showPending };
}
