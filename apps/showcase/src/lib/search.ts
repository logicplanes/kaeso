export interface SearchEntry {
  parts: string[];
  props: string[];
  description: string;
  examples: { title: string; description?: string; text: string }[];
}

export type SearchIndex = Record<string, SearchEntry>;

export interface MatchLabels {
  part: string;
  prop: string;
  description: string;
  example: string;
  named: (label: string, name: string) => string;
}

type Translate = (english: string) => string;

// Below this length a word is still being typed: naming a reason for it would change most results' hints on every key.
const REASON_MIN = 3;

const exampleTexts = (e: SearchEntry["examples"][number], t: Translate) => [
  e.title,
  t(e.title),
  e.text,
  ...(e.description ? [e.description, t(e.description)] : []),
];

export function searchKeywords(entry: SearchEntry, t: Translate): string[] {
  return [...entry.parts, ...entry.props, entry.description, t(entry.description), ...entry.examples.flatMap((e) => exampleTexts(e, t))];
}

export function matchReason(label: string, entry: SearchEntry, query: string, t: Translate, labels: MatchLabels): string | null {
  const name = label.toLowerCase();
  const words = query
    .toLowerCase()
    .split(/\s+/)
    .filter((w) => w.length >= REASON_MIN && !name.includes(w));
  if (!words.length) return null;
  const candidates: [reason: string, texts: string[]][] = [
    ...entry.parts.map((p): [string, string[]] => [labels.named(labels.part, p), [p]]),
    ...entry.props.map((p): [string, string[]] => [labels.named(labels.prop, p), [p]]),
    [labels.description, [entry.description, t(entry.description)]],
    ...entry.examples.map((e): [string, string[]] => [labels.named(labels.example, t(e.title)), exampleTexts(e, t)]),
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
