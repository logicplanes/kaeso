export type DiffKind = "same" | "removed" | "added";

export interface DiffLine {
  kind: DiffKind;
  text: string;
  oldNumber?: number | undefined;
  newNumber?: number | undefined;
  changes?: [number, number][] | undefined;
}

export interface DiffStats {
  added: number;
  removed: number;
}

// Past this many cells the quadratic table costs more memory than a readable diff is worth.
const MAX_CELLS = 4_000_000;
const MAX_WORD_TOKENS = 400;
const WORD = /\w+|\s+|[^\w\s]/g;

function lcsPairs<T>(a: readonly T[], b: readonly T[]): [number, number][] | null {
  if (a.length * b.length > MAX_CELLS) return null;
  const width = b.length + 1;
  const table = new Uint32Array((a.length + 1) * width);
  for (let i = a.length - 1; i >= 0; i--)
    for (let j = b.length - 1; j >= 0; j--)
      table[i * width + j] =
        a[i] === b[j] ? (table[(i + 1) * width + j + 1] ?? 0) + 1 : Math.max(table[(i + 1) * width + j] ?? 0, table[i * width + j + 1] ?? 0);
  const pairs: [number, number][] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      pairs.push([i, j]);
      i++;
      j++;
    } else if ((table[(i + 1) * width + j] ?? 0) >= (table[i * width + j + 1] ?? 0)) i++;
    else j++;
  }
  return pairs;
}

function wordChanges(before: string, after: string): [[number, number][], [number, number][]] | null {
  const a = before.match(WORD) ?? [];
  const b = after.match(WORD) ?? [];
  if (a.length > MAX_WORD_TOKENS || b.length > MAX_WORD_TOKENS) return null;
  const pairs = lcsPairs(a, b) ?? [];
  const keptText = pairs.map(([i]) => a[i] ?? "").join("");
  const longest = Math.max(before.replace(/\s/g, "").length, after.replace(/\s/g, "").length);
  if (longest === 0 || (!/\w/.test(keptText) && keptText.replace(/\s/g, "").length / longest < 0.4)) return null;
  const ranges = (tokens: string[], keptAt: Set<number>) => {
    const line = tokens.join("");
    const out: [number, number][] = [];
    let at = 0;
    tokens.forEach((token, k) => {
      const end = at + token.length;
      if (!keptAt.has(k) && token.trim()) {
        const last = out[out.length - 1];
        if (last && !line.slice(last[1], at).trim()) last[1] = end;
        else out.push([at, end]);
      }
      at = end;
    });
    return out;
  };
  return [ranges(a, new Set(pairs.map(([i]) => i))), ranges(b, new Set(pairs.map(([, j]) => j)))];
}

function markWords(removed: DiffLine[], added: DiffLine[]) {
  const n = Math.min(removed.length, added.length);
  for (let k = 0; k < n; k++) {
    const r = removed[k] as DiffLine;
    const a = added[k] as DiffLine;
    const words = wordChanges(r.text, a.text);
    if (!words) continue;
    r.changes = words[0];
    a.changes = words[1];
  }
}

export function diffLines(before: string, after: string): DiffLine[] {
  const a = before === "" ? [] : before.replace(/\n$/, "").split("\n");
  const b = after === "" ? [] : after.replace(/\n$/, "").split("\n");
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  const midA = a.slice(head, a.length - tail);
  const midB = b.slice(head, b.length - tail);
  const pairs = (lcsPairs(midA, midB) ?? []).map(([i, j]): [number, number] => [i + head, j + head]);
  const anchors: [number, number][] = [
    ...Array.from({ length: head }, (_, k): [number, number] => [k, k]),
    ...pairs,
    ...Array.from({ length: tail }, (_, k): [number, number] => [a.length - tail + k, b.length - tail + k]),
    [a.length, b.length],
  ];
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  for (const [ai, bj] of anchors) {
    const removed = a.slice(i, ai).map((text, k): DiffLine => ({ kind: "removed", text, oldNumber: i + k + 1 }));
    const added = b.slice(j, bj).map((text, k): DiffLine => ({ kind: "added", text, newNumber: j + k + 1 }));
    markWords(removed, added);
    out.push(...removed, ...added);
    if (ai < a.length) out.push({ kind: "same", text: a[ai] as string, oldNumber: ai + 1, newNumber: bj + 1 });
    i = ai + 1;
    j = bj + 1;
  }
  return out;
}

export function diffStats(lines: readonly DiffLine[]): DiffStats {
  return {
    added: lines.filter((l) => l.kind === "added").length,
    removed: lines.filter((l) => l.kind === "removed").length,
  };
}

export function largestLineNumber(lines: readonly DiffLine[]): number {
  let largest = 1;
  for (const l of lines) largest = Math.max(largest, l.oldNumber ?? 0, l.newNumber ?? 0);
  return largest;
}

export function unifiedDiff(lines: readonly DiffLine[]): string {
  return lines.map((l) => `${l.kind === "added" ? "+" : l.kind === "removed" ? "-" : " "} ${l.text}`).join("\n");
}

export type DiffRow = { type: "line"; line: DiffLine; index: number } | { type: "fold"; from: number; to: number };

export function foldUnchanged(lines: readonly DiffLine[], context: number, open: ReadonlySet<number>): DiffRow[] {
  const near = Array.from({ length: lines.length }, () => false);
  lines.forEach((l, k) => {
    if (l.kind === "same") return;
    for (let d = Math.max(0, k - context); d <= Math.min(lines.length - 1, k + context); d++) near[d] = true;
  });
  const rows: DiffRow[] = [];
  let k = 0;
  while (k < lines.length) {
    if (near[k] || lines[k]?.kind !== "same") {
      rows.push({ type: "line", line: lines[k] as DiffLine, index: k });
      k++;
      continue;
    }
    let end = k;
    while (end < lines.length && !near[end]) end++;
    if (end - k < 2 || open.has(k)) for (let m = k; m < end; m++) rows.push({ type: "line", line: lines[m] as DiffLine, index: m });
    else rows.push({ type: "fold", from: k, to: end });
    k = end;
  }
  return rows;
}
