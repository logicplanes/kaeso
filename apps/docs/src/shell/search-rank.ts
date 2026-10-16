import type { SearchEntry } from "~/markdown/ast";

const words = (q: string) => q.toLowerCase().split(/\s+/).filter(Boolean);
const has = (text: string, ws: string[]) => {
  const hay = text.toLowerCase();
  return ws.every((w) => hay.includes(w));
};

export function rankPages(entries: SearchEntry[], query: string): SearchEntry[] {
  const ws = words(query);
  if (!ws.length) return entries;
  const score = (e: SearchEntry) => (has(e.title, ws) ? (e.title.toLowerCase().startsWith(ws[0] ?? "") ? 0 : 1) : has(e.description, ws) ? 2 : 3);
  return entries
    .map((e, i) => ({ e, i, s: score(e) }))
    .sort((a, b) => a.s - b.s || a.i - b.i)
    .map((x) => x.e);
}

export function matchHeadings(entries: SearchEntry[], query: string): { entry: SearchEntry; id: string; label: string }[] {
  const ws = words(query);
  if (!ws.length) return [];
  return entries.flatMap((entry) => entry.headings.filter((h) => has(h.label, ws)).map((h) => ({ entry, ...h })));
}
