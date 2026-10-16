export interface Frontmatter {
  title: string;
  description: string;
  order?: number;
  nav?: string;
}

const FIELDS = new Set(["title", "description", "order", "nav"]);

export interface Parsed {
  data: Frontmatter;
  body: string;
  lines: number;
  problems: string[];
}

function unquote(raw: string): string {
  const v = raw.trim();
  if (v.length >= 2 && ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'")))) {
    const inner = v.slice(1, -1);
    return v.startsWith('"') ? inner.replace(/\\"/g, '"').replace(/\\\\/g, "\\") : inner.replace(/''/g, "'");
  }
  return v;
}

const DESCRIPTION_MAX = 140;

const LATIN_SENTENCE_END = /[.!?]\s+\S/;
const CJK_SENTENCE_END = /[。！？]\s*[^\s。！？」』）)"”]/u;

function hasSecondSentence(description: string): boolean {
  return LATIN_SENTENCE_END.test(description) || CJK_SENTENCE_END.test(description);
}

export function parseFrontmatter(source: string): Parsed {
  const problems: string[] = [];
  const text = (source.charCodeAt(0) === 0xfeff ? source.slice(1) : source).replace(/\r\n?/g, "\n");
  const match = /^---\n([\s\S]*?)\n---[ \t]*(?:\n|$)/.exec(text);
  if (!match) {
    return {
      data: { title: "", description: "" },
      body: text,
      lines: 0,
      problems: ["starts without frontmatter: add `---`, `title: …`, `description: …` and `---` at the top"],
    };
  }
  const raw: Record<string, string> = {};
  for (const [i, line] of (match[1] ?? "").split("\n").entries()) {
    if (!line.trim() || line.trim().startsWith("#")) continue;
    const kv = /^([A-Za-z][\w-]*):(.*)$/.exec(line);
    if (!kv) {
      problems.push(`frontmatter line ${i + 2} is not \`key: value\`: ${line.trim()}`);
      continue;
    }
    const key = kv[1] ?? "";
    if (!FIELDS.has(key)) {
      problems.push(`frontmatter has an unknown field \`${key}\` (known: ${[...FIELDS].join(", ")})`);
      continue;
    }
    if (key in raw) problems.push(`frontmatter sets \`${key}\` twice`);
    raw[key] = unquote(kv[2] ?? "");
  }
  const data: Frontmatter = { title: raw.title ?? "", description: raw.description ?? "" };
  if (!data.title) problems.push("frontmatter has no `title`");
  if (!data.description) problems.push("frontmatter has no `description`: one short sentence about the page");
  else {
    if (data.description.length > DESCRIPTION_MAX)
      problems.push(`the description is ${data.description.length} characters; keep it to one short sentence (at most ${DESCRIPTION_MAX})`);
    if (hasSecondSentence(data.description)) problems.push("the description has more than one sentence; join them with a semicolon or cut one");
  }
  if (raw.order !== undefined) {
    if (!/^-?\d+$/.test(raw.order)) problems.push(`\`order\` must be a whole number, not "${raw.order}"`);
    else data.order = Number(raw.order);
  }
  if (raw.nav !== undefined) {
    if (!raw.nav) problems.push("`nav` is empty: leave it out to use the title");
    else data.nav = raw.nav;
  }
  const lines = (match[0].match(/\n/g) ?? []).length;
  return { data, body: text.slice(match[0].length), lines, problems };
}
