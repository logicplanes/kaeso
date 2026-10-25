import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { KEY, SLACK, lineAt, literal, quote, root, sentences, shown, startsLiteral, translations, width } from "~/test/copy";

const ROW = 60;
const ROW_BESIDE_INPUT = 50;
const HEADER = 80;
const HEADER_BESIDE_ACTIONS = 70;
const DIALOG = 100;
const EMPTY = 90;

const TAG = /<(SettingsRow|SettingsSection|SettingsPage|OrgPage|AppFrame\.Settings|Dialog|AlertDialog|Empty)(?![\w.])/g;
const HEADERS = new Set(["SettingsSection", "SettingsPage", "OrgPage", "AppFrame.Settings"]);
const DIALOGS = new Set(["Dialog", "AlertDialog"]);
const STEP = "StepDialog step";

interface Description {
  where: string;
  component: string;
  text: string;
  limit: number;
  oneSentence: boolean;
  key?: string;
}

function expression(src: string, at: number, file: string, component: string, limit: number): Description[] {
  const found: Description[] = [];
  const oneSentence = !DIALOGS.has(component) && component !== STEP;
  let j = at;
  let depth = 0;
  let key: string | undefined;
  while (j < src.length) {
    if (startsLiteral(src, j)) {
      const lit = literal(src, j);
      if (KEY.test(lit.text)) key = lit.text;
      else {
        found.push({ where: `${file}:${lineAt(src, j)}`, component, text: lit.text, limit, oneSentence, ...(key ? { key } : {}) });
        key = undefined;
      }
      j = lit.end;
      continue;
    }
    const c = src[j] ?? "";
    if ("([{".includes(c)) depth++;
    else if (")]}".includes(c)) depth--;
    if (depth < 0 || (depth === 0 && c === ",")) break;
    j++;
  }
  return found;
}

function steps(file: string, src: string): Description[] {
  const found: Description[] = [];
  const open: { keys: Set<string>; descriptions: number[] }[] = [];
  for (let i = 0; i < src.length; i++) {
    if (startsLiteral(src, i)) {
      i = literal(src, i).end - 1;
      continue;
    }
    if (src[i] === "{") open.push({ keys: new Set(), descriptions: [] });
    else if (src[i] === "}") {
      const object = open.pop();
      if (object?.keys.has("content")) for (const at of object.descriptions) found.push(...expression(src, at, file, STEP, DIALOG));
    } else {
      const property = /^([A-Za-z]+): /.exec(src.slice(i, i + 40));
      const object = open.at(-1);
      if (!property?.[1] || !object || !/[\s{,]/.test(src[i - 1] ?? "")) continue;
      object.keys.add(property[1]);
      if (property[1] === "description") object.descriptions.push(i + property[0].length);
      i += property[0].length - 1;
    }
  }
  return found;
}

function descriptions(file: string): Description[] {
  const src = readFileSync(join(root, file), "utf8");
  const found: Description[] = steps(file, src);
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
    const limit = DIALOGS.has(component)
      ? DIALOG
      : component === "Empty"
        ? EMPTY
        : HEADERS.has(component)
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
      const oneSentence = !DIALOGS.has(component);
      found.push({ where: `${file}:${lineAt(src, at)}`, component, text: literal(src, at).text, limit, oneSentence });
      continue;
    }
    found.push(...expression(src, at + 1, file, component, limit));
  }
  return found;
}

const files = readdirSync(join(root, "src"), { recursive: true })
  .map((f) => `src/${String(f)}`)
  .filter((f) => f.endsWith(".tsx"))
  .sort();
const all = files.flatMap(descriptions);
const messages = translations();

describe("descriptions", () => {
  it("finds the descriptions it checks", () => {
    const found = (component: string, file: string) => all.some((d) => d.component === component && d.where.startsWith(file));
    expect(found("SettingsRow", "src/features/account/profile-page.tsx")).toBe(true);
    expect(found("AlertDialog", "src/features/account/keys-page.tsx")).toBe(true);
    expect(found("Dialog", "src/shell/shortcut-sheet.tsx")).toBe(true);
    expect(found(STEP, "src/features/org/invite-dialog.tsx")).toBe(true);
    expect(found("Empty", "src/auth/org-access.tsx")).toBe(true);
  });
  it("are one sentence", () => {
    expect(all.filter((d) => d.oneSentence && sentences(shown(d.text)) > 1).map((d) => `${d.where} ${d.component}: ${d.text}`)).toEqual([]);
  });
  it("fit their lines", () => {
    expect(all.filter((d) => shown(d.text).length > d.limit).map((d) => `${d.where} ${d.component} (${shown(d.text).length} > ${d.limit}): ${d.text}`)).toEqual(
      [],
    );
  });
  it("fit their lines in German and Chinese, as one sentence", () => {
    const long = all.flatMap((d) =>
      Object.entries(messages).flatMap(([locale, catalog]) => {
        const text = d.key ? catalog[d.key] : undefined;
        if (!text) return [];
        const max = Math.floor(d.limit * SLACK);
        const problems = [];
        if (width(shown(text)) > max) problems.push(`${d.where} ${locale} ${d.key} (${width(shown(text))} > ${max}): ${text}`);
        if (d.oneSentence && sentences(shown(text)) > 1) problems.push(`${d.where} ${locale} ${d.key} is more than one sentence: ${text}`);
        return problems;
      }),
    );
    expect(long).toEqual([]);
  });
});
