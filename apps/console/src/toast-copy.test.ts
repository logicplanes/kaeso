import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { KEY, SLACK, literal, root, sentences, shown, startsLiteral, translations, width } from "~/test/copy";

const LIMIT = 60;
const TOAST = /\btoast(\.(?:success|error|warning))?\(|\b(fallback): /g;

interface Toast {
  file: string;
  error: boolean;
  key: string;
  english: string;
}

function messages(src: string, at: number): { key: string; english: string }[] {
  const found: { key: string; english: string }[] = [];
  let depth = 0;
  for (let j = at; j < src.length; j++) {
    if (startsLiteral(src, j)) {
      j = literal(src, j).end - 1;
      continue;
    }
    const c = src[j] ?? "";
    if (depth === 0 && /^\bt\(\s*"/.test(src.slice(j, j + 40)) && !/[\w.]/.test(src[j - 1] ?? "")) {
      const keyAt = src.indexOf('"', j);
      const key = literal(src, keyAt);
      const englishAt = key.end + (/^,\s*/.exec(src.slice(key.end))?.[0].length ?? 0);
      if (KEY.test(key.text) && src[englishAt] === '"') found.push({ key: key.text, english: literal(src, englishAt).text });
    }
    if ("([{".includes(c)) depth++;
    else if (")]}".includes(c)) depth--;
    if (depth < 0 || (depth === 0 && c === ",")) break;
  }
  return found;
}

const toasts: Toast[] = readdirSync(join(root, "src"), { recursive: true })
  .map(String)
  .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f) && !f.startsWith("api/generated") && !f.startsWith("test/"))
  .flatMap((file) => {
    const src = readFileSync(join(root, "src", file), "utf8");
    return [...src.matchAll(TOAST)].flatMap((m) => messages(src, m.index + m[0].length).map((x) => ({ file, error: m[1] === ".error" || !!m[2], ...x })));
  });

describe("toasts", () => {
  const { de = {}, "zh-CN": zh = {} } = translations();

  it("are found in the sources", () => {
    expect(toasts.length).toBeGreaterThan(50);
    expect(toasts.some((x) => x.key === "auth.gone.notMember")).toBe(true);
  });

  it.each(toasts.filter((x) => !/^\{\w+\}: \{message\}$/.test(x.english)))("$key ends with a period only when it is an error", ({ error, key, english }) => {
    expect(english.endsWith("."), english).toBe(error);
    expect((de[key] ?? "").endsWith("."), de[key]).toBe(error);
    expect((zh[key] ?? "").endsWith("。"), zh[key]).toBe(error);
  });

  it("are one sentence that fits on one line", () => {
    const max = Math.floor(LIMIT * SLACK);
    const problems = toasts.flatMap(({ file, key, english }) => [
      ...(shown(english).length > LIMIT ? [`${file} ${key} (${shown(english).length} > ${LIMIT}): ${english}`] : []),
      ...[
        ["en", english],
        ["de", de[key] ?? ""],
        ["zh-CN", zh[key] ?? ""],
      ].flatMap(([locale, text = ""]) => [
        ...(locale !== "en" && width(shown(text)) > max ? [`${file} ${locale} ${key} (${width(shown(text))} > ${max}): ${text}`] : []),
        ...(sentences(shown(text)) > 1 ? [`${file} ${locale} ${key} is more than one sentence: ${text}`] : []),
      ]),
    ]);
    expect(problems).toEqual([]);
  });
});
