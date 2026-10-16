import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const PLACEHOLDER = 12;
const ROW = 60;
const ROW_BESIDE_INPUT = 50;
const HEADER = 80;
const HEADER_BESIDE_ACTIONS = 70;

const TAG = /<(SettingsRow|SettingsSection|SettingsPage|OrgPage|AppFrame\.Settings)\b/g;
const HEADERS = new Set(["SettingsSection", "SettingsPage", "OrgPage", "AppFrame.Settings"]);

interface Description {
  where: string;
  component: string;
  text: string;
  limit: number;
  key?: string;
}

const KEY = /^[a-z][a-zA-Z0-9]*(\.[a-z][a-zA-Z0-9]*)+$/;

const root = join(__dirname, "..");
const quote = (c: string | undefined) => c === '"' || c === "'" || c === "`";
const startsLiteral = (src: string, i: number) => quote(src[i]) && !/\w/.test(src[i - 1] ?? "");

function literal(src: string, i: number): { text: string; end: number } {
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

const lineAt = (src: string, i: number) => src.slice(0, i).split("\n").length;

function descriptions(file: string): Description[] {
  const src = readFileSync(join(root, file), "utf8");
  const found: Description[] = [];
  for (const tag of src.matchAll(TAG)) {
    const component = tag[1] ?? "";
    const start = tag.index + tag[0].length;
    let end = start;
    let depth = 0;
    const attrs: { name: string; at: number }[] = [];
    while (end < src.length && !(depth === 0 && src[end] === ">")) {
      if (startsLiteral(src, end)) {
        end = literal(src, end).end;
        continue;
      }
      if (src[end] === "{") depth++;
      else if (src[end] === "}") depth--;
      else if (depth === 0) {
        const attr = /^([A-Za-z]+)=/.exec(src.slice(end, end + 40));
        if (attr?.[1] && /\s/.test(src[end - 1] ?? "")) attrs.push({ name: attr[1], at: end + attr[0].length });
      }
      end++;
    }
    const opening = src.slice(start, end);
    const limit = HEADERS.has(component)
      ? attrs.some((a) => a.name === "actions")
        ? HEADER_BESIDE_ACTIONS
        : HEADER
      : /<Input\b/.test(opening)
        ? ROW_BESIDE_INPUT
        : ROW;
    const description = attrs.find((a) => a.name === "description");
    if (!description) continue;
    const at = description.at;
    if (quote(src[at])) {
      found.push({ where: `${file}:${lineAt(src, at)}`, component, text: literal(src, at).text, limit });
      continue;
    }
    let j = at + 1;
    let d = 1;
    let key: string | undefined;
    while (j < src.length && d) {
      if (startsLiteral(src, j)) {
        const lit = literal(src, j);
        if (KEY.test(lit.text)) key = lit.text;
        else {
          found.push({ where: `${file}:${lineAt(src, j)}`, component, text: lit.text, limit, ...(key ? { key } : {}) });
          key = undefined;
        }
        j = lit.end;
        continue;
      }
      if (src[j] === "{") d++;
      else if (src[j] === "}") d--;
      j++;
    }
  }
  return found;
}

const sentences = (text: string) => 1 + (text.replace(/\b(e\.g|i\.e|etc|vs)\./g, "$1").match(/[.!?]["”’)]?\s+\S/g)?.length ?? 0);

const files = readdirSync(join(root, "src"), { recursive: true })
  .map((f) => `src/${String(f)}`)
  .filter((f) => f.endsWith(".tsx"))
  .sort();
const all = files.flatMap(descriptions);

const catalog = (locale: string): Record<string, string> =>
  Object.assign(
    {},
    ...readdirSync(join(root, "src/i18n/locales", locale)).map((f) => JSON.parse(readFileSync(join(root, "src/i18n/locales", locale, f), "utf8"))),
  );
const translations = { de: catalog("de"), "zh-CN": catalog("zh-CN") };
const SLACK = 1.15;
const width = (text: string) => [...text].reduce((w, c) => w + (/[\u3000-\u9fff\uff00-\uffef]/.test(c) ? 2 : 1), 0);

describe("settings descriptions", () => {
  it("finds the descriptions it checks", () => {
    expect(all.some((d) => d.where.startsWith("src/features/account/profile-page.tsx"))).toBe(true);
  });
  it("are one sentence", () => {
    expect(all.filter((d) => sentences(d.text) > 1).map((d) => `${d.where} ${d.component}: ${d.text}`)).toEqual([]);
  });
  it("fit on one line", () => {
    expect(all.filter((d) => d.text.length > d.limit).map((d) => `${d.where} ${d.component} (${d.text.length} > ${d.limit}): ${d.text}`)).toEqual([]);
  });
  it("fit on one line in German and Chinese, as one sentence", () => {
    const long = all.flatMap((d) =>
      Object.entries(translations).flatMap(([locale, messages]) => {
        const text = d.key ? messages[d.key] : undefined;
        if (!text) return [];
        const max = Math.floor(d.limit * SLACK);
        const problems = [];
        if (width(text) > max) problems.push(`${d.where} ${locale} ${d.key} (${width(text)} > ${max}): ${text}`);
        if (sentences(text) > 1) problems.push(`${d.where} ${locale} ${d.key} is more than one sentence: ${text}`);
        return problems;
      }),
    );
    expect(long).toEqual([]);
  });
});
