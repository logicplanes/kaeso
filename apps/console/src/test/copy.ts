import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

export const root = join(__dirname, "../..");
const PLACEHOLDER = 12;
export const SLACK = 1.15;
export const KEY = /^[a-z][a-zA-Z0-9]*(\.[a-z][a-zA-Z0-9]*)+$/;

export const quote = (c: string | undefined) => c === '"' || c === "'" || c === "`";
export const startsLiteral = (src: string, i: number) => quote(src[i]) && !/\w/.test(src[i - 1] ?? "");
export const lineAt = (src: string, i: number) => src.slice(0, i).split("\n").length;

export function literal(src: string, i: number): { text: string; end: number } {
  const q = src[i];
  let text = "";
  let j = i + 1;
  while (j < src.length && src[j] !== q) {
    if (src[j] === "\\") {
      text += src[j + 1] ?? "";
      j += 2;
    } else if (q === "`" && src.startsWith("${", j)) {
      let depth = 1;
      j += 2;
      while (j < src.length && depth) {
        if (src[j] === "{") depth++;
        else if (src[j] === "}") depth--;
        j++;
      }
      text += "#".repeat(PLACEHOLDER);
    } else text += src[j++];
  }
  return { text, end: j + 1 };
}

export function shown(message: string): string {
  let out = "";
  let i = 0;
  while (i < message.length) {
    const m = message[i] === "{" ? /^\{\s*\w+\s*(,\s*(?:plural|select)\s*,)?/.exec(message.slice(i)) : null;
    if (!m) {
      out += message[i++];
      continue;
    }
    i += m[0].length;
    if (!m[1]) {
      out += "#".repeat(PLACEHOLDER);
      i = message.indexOf("}", i) + 1 || message.length;
      continue;
    }
    let longest = "";
    while (i < message.length && message[i] !== "}") {
      if (message[i] !== "{") {
        i++;
        continue;
      }
      let depth = 1;
      let j = i + 1;
      while (j < message.length && depth) {
        if (message[j] === "{") depth++;
        else if (message[j] === "}") depth--;
        j++;
      }
      const branch = shown(message.slice(i + 1, j - 1));
      if (branch.length > longest.length) longest = branch;
      i = j;
    }
    out += longest;
    i++;
  }
  return out;
}

export const sentences = (text: string) =>
  1 + (text.replace(/\b(e\.g|i\.e|etc|vs)\./g, "$1").match(/[.!?]["”’)]?\s+\S|[。！？][”’）]?[^\s”’）]/gu)?.length ?? 0);
export const width = (text: string) => [...text].reduce((w, c) => w + (/[\u3000-\u9fff\uff00-\uffef]/.test(c) ? 2 : 1), 0);

const catalog = (locale: string): Record<string, string> =>
  Object.assign(
    {},
    ...readdirSync(join(root, "src/i18n/locales", locale)).map((f) => JSON.parse(readFileSync(join(root, "src/i18n/locales", locale, f), "utf8"))),
  );
export const translations = (): Record<string, Record<string, string>> => ({ de: catalog("de"), "zh-CN": catalog("zh-CN") });
