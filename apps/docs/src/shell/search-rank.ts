import type { SearchEntry } from "~/markdown/ast";

const words = (q: string) => q.toLowerCase().split(/\s+/).filter(Boolean);
const has = (text: string, ws: string[]) => {
  const hay = text.toLowerCase();
  return ws.every((w) => hay.includes(w));
};

const MENTIONED = 3;

function scored(entries: SearchEntry[], query: string): { e: SearchEntry; s: number }[] {
  const ws = words(query);
  if (!ws.length) return entries.map((e) => ({ e, s: 0 }));
  const score = (e: SearchEntry) => (has(e.title, ws) ? (e.title.toLowerCase().startsWith(ws[0] ?? "") ? 0 : 1) : has(e.description, ws) ? 2 : MENTIONED);
  return entries
    .map((e, i) => ({ e, i, s: score(e) }))
    .sort((a, b) => a.s - b.s || a.i - b.i)
    .map(({ e, s }) => ({ e, s }));
}

export function rankPages(entries: SearchEntry[], query: string): SearchEntry[] {
  return scored(entries, query).map((x) => x.e);
}

export function splitPages(entries: SearchEntry[], query: string): { named: SearchEntry[]; mentioned: SearchEntry[] } {
  const all = scored(entries, query);
  return { named: all.filter((x) => x.s < MENTIONED).map((x) => x.e), mentioned: all.filter((x) => x.s === MENTIONED).map((x) => x.e) };
}

export function matchHeadings(entries: SearchEntry[], query: string): { entry: SearchEntry; id: string; label: string }[] {
  const ws = words(query);
  if (!ws.length) return [];
  return entries.flatMap((entry) => entry.headings.filter((h) => has(h.label, ws)).map((h) => ({ entry, ...h })));
}
