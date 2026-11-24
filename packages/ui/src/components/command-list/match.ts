const SEPARATORS = /[\s\-_./:@]/g;
const SEPARATOR = /[\s\-_./:@]/;

interface Prepared {
  lower: string[];
  compact: string[];
}

function wordScore(word: string, compactWord: string, field: string, compactField: string): number {
  if (field === word) return 100;
  if (field.startsWith(word)) return 80;
  const at = field.indexOf(word);
  if (at > 0) return SEPARATOR.test(field[at - 1] ?? "") ? 70 : 50;
  if (compactWord && compactField.includes(compactWord)) return 40;
  if (word.length < 3) return 0;
  let next = 0;
  let last = -1;
  let gaps = 0;
  for (let i = 0; i < field.length && next < word.length; i++) {
    if (field[i] !== word[next]) continue;
    if (last >= 0 && i !== last + 1) gaps += 1;
    last = i;
    next += 1;
  }
  return next < word.length ? 0 : Math.max(1, 30 - gaps * 3);
}

function prepare(fields: readonly string[]): Prepared {
  const lower = fields.map((f) => f.toLowerCase());
  return { lower, compact: lower.map((f) => f.replace(SEPARATORS, "")) };
}

const split = (query: string) =>
  query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => ({ word, compact: word.replace(SEPARATORS, "") }));

let lastQuery = "";
let lastWords: { word: string; compact: string }[] = [];
function wordsOf(query: string) {
  if (query !== lastQuery) {
    lastQuery = query;
    lastWords = split(query);
  }
  return lastWords;
}

// Whether every item that `before` didn't match also fails `after`: the same words with the last one typed further.
// A word of three letters or more that matches no field by substring, compact substring or letters in order can't
// match once it's longer; a shorter word skips the letters-in-order test, so a longer one may still match.
let narrowed: { before: string; after: string; holds: boolean } = { before: "", after: "", holds: false };
function narrows(before: string, after: string) {
  if (narrowed.before !== before || narrowed.after !== after) {
    const a = split(before);
    const b = split(after);
    const last = a.length - 1;
    const holds =
      a.length > 0 &&
      a.length === b.length &&
      a.every(({ word, compact }, i) => (i < last ? word === b[i]?.word : word.length >= 3 && compact !== "" && !!b[i]?.word.startsWith(word)));
    narrowed = { before, after, holds };
  }
  return narrowed.holds;
}

function score(query: string, fields: Prepared): number {
  const words = wordsOf(query);
  if (words.length === 0) return 1;
  let total = 0;
  for (const { word, compact } of words) {
    let best = 0;
    for (let i = 0; i < fields.lower.length; i++) {
      best = Math.max(best, wordScore(word, compact, fields.lower[i]!, fields.compact[i]!) * (i === 0 ? 1 : 0.9));
    }
    if (best === 0) return 0;
    total += best;
  }
  return total / words.length;
}

export function commandMatch(query: string, fields: readonly string[]): number {
  return score(query, prepare(fields));
}

export interface Matchable {
  label: string;
  keywords?: readonly string[] | undefined;
}

// Thousands of models are scored on every keystroke, often twice (a picker's provider counts, then the list):
// each item's fields are lowercased once, and its score for the last query is kept.
const prepared = new WeakMap<Matchable, Prepared & { query?: string | undefined; score?: number | undefined }>();

export function itemMatch(item: Matchable, query: string): number {
  let fields = prepared.get(item);
  if (!fields) {
    fields = prepare([item.label, ...(item.keywords ?? [])]);
    prepared.set(item, fields);
  }
  if (fields.query !== query) {
    if (!(fields.score === 0 && fields.query !== undefined && narrows(fields.query, query))) fields.score = score(query, fields);
    fields.query = query;
  }
  return fields.score ?? 0;
}
