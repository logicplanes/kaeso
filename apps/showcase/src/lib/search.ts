/** What the search finds a component page by beyond its name; written by scripts/gen-pages.mjs. */
export interface SearchEntry {
  parts: string[];
  props: string[];
  description: string;
  examples: { title: string; description?: string; text: string }[];
}

export type SearchIndex = Record<string, SearchEntry>;

/** The words a match reason starts with, in the page's language. */
export interface MatchLabels {
  part: string;
  prop: string;
  description: string;
  example: string;
  /** ": ", or "：" in Chinese. */
  colon: string;
}

type Translate = (english: string) => string;

const exampleTexts = (e: SearchEntry["examples"][number], t: Translate) => [
  e.title,
  t(e.title),
  e.text,
  ...(e.description ? [e.description, t(e.description)] : []),
];

/** Every text a page is found by, in English and in the page's language: the palette matches a query against these. */
export function searchKeywords(entry: SearchEntry, t: Translate): string[] {
  return [...entry.parts, ...entry.props, entry.description, t(entry.description), ...entry.examples.flatMap((e) => exampleTexts(e, t))];
}

/**
 * Why a page matches a query its name doesn't explain: the part, prop, description or example that holds the most of the
 * query's other words, a name equal to a word first, then the earlier kind. Null when the name holds every word, or nothing else holds any.
 */
export function matchReason(label: string, entry: SearchEntry, query: string, t: Translate, labels: MatchLabels): string | null {
  const name = label.toLowerCase();
  const words = query
    .toLowerCase()
    .split(/\s+/)
    .filter((w) => w && !name.includes(w));
  if (!words.length) return null;
  const candidates: [reason: string, texts: string[]][] = [
    ...entry.parts.map((p): [string, string[]] => [`${labels.part}${labels.colon}${p}`, [p]]),
    ...entry.props.map((p): [string, string[]] => [`${labels.prop}${labels.colon}${p}`, [p]]),
    [labels.description, [entry.description, t(entry.description)]],
    ...entry.examples.map((e): [string, string[]] => [`${labels.example}${labels.colon}${t(e.title)}`, exampleTexts(e, t)]),
  ];
  let best: string | null = null;
  let most = 0;
  for (const [reason, texts] of candidates) {
    const hay = texts.join(" ").toLowerCase();
    // A name that is exactly a query word (the prop pressed for "pressed") beats one that only contains it (onPressedChange).
    const held = words.filter((w) => hay.includes(w)).length + (texts.some((x) => words.includes(x.toLowerCase())) ? 0.5 : 0);
    if (held > most) {
      best = reason;
      most = held;
    }
  }
  return best;
}
