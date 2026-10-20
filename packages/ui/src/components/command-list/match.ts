const SEPARATORS = /[\s\-_./:@]/g;
const SEPARATOR = /[\s\-_./:@]/;

function wordScore(word: string, field: string): number {
  if (field === word) return 100;
  if (field.startsWith(word)) return 80;
  const at = field.indexOf(word);
  if (at > 0) return SEPARATOR.test(field[at - 1] ?? "") ? 70 : 50;
  const compactWord = word.replace(SEPARATORS, "");
  if (compactWord && field.replace(SEPARATORS, "").includes(compactWord)) return 40;
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

export function commandMatch(query: string, fields: readonly string[]): number {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return 1;
  const lower = fields.map((f) => f.toLowerCase());
  let total = 0;
  for (const word of words) {
    let best = 0;
    lower.forEach((field, i) => {
      best = Math.max(best, wordScore(word, field) * (i === 0 ? 1 : 0.9));
    });
    if (best === 0) return 0;
    total += best;
  }
  return total / words.length;
}
