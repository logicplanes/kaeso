import type { CodeToken } from "./highlighter.js";

export const INDENT = /^[ \t]+/;

export function dropIndent(tokens: CodeToken[], n: number): CodeToken[] {
  let left = n;
  return tokens.flatMap((t) => {
    if (left === 0) return [t];
    const cut = Math.min(left, t.content.length);
    left -= cut;
    return cut === t.content.length ? [] : [{ ...t, content: t.content.slice(cut) }];
  });
}
