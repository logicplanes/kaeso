import type { CodeToken } from "./highlighter.js";

export type WordPart = Pick<CodeToken, "content" | "className">;

export function splitWords(tokens: readonly WordPart[]): (string | WordPart[])[] {
  const out: (string | WordPart[])[] = [];
  for (const token of tokens) {
    for (const part of token.content.split(/(\s+)/)) {
      if (!part) continue;
      const last = out[out.length - 1];
      if (/^\s/.test(part)) {
        if (typeof last === "string") out[out.length - 1] = last + part;
        else out.push(part);
      } else if (Array.isArray(last)) last.push({ content: part, className: token.className });
      else out.push([{ content: part, className: token.className }]);
    }
  }
  return out;
}
