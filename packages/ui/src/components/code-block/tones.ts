export const TONES = {
  text: { color: "var(--k-text)", className: "" },
  keyword: { color: "var(--k-code-keyword)", className: "text-code-keyword" },
  string: { color: "var(--k-code-string)", className: "text-code-string" },
  constant: { color: "var(--k-code-constant)", className: "text-code-constant" },
  comment: { color: "var(--k-code-comment)", className: "text-code-comment" },
  function: { color: "var(--k-code-function)", className: "text-code-function" },
  type: { color: "var(--k-code-type)", className: "text-code-type" },
  property: { color: "var(--k-code-property)", className: "text-code-property" },
  tag: { color: "var(--k-code-tag)", className: "text-code-tag" },
  attribute: { color: "var(--k-code-attribute)", className: "text-code-attribute" },
  variable: { color: "var(--k-code-variable)", className: "text-code-variable" },
  punctuation: { color: "var(--k-code-punctuation)", className: "text-code-punctuation" },
  inserted: { color: "var(--k-code-inserted)", className: "text-code-inserted" },
  deleted: { color: "var(--k-code-deleted)", className: "text-code-deleted" },
} as const;

/** A role of Kaeso's syntax palette, drawn with the `--k-code-<role>` token and the `text-code-<role>` class. */
export type CodeTone = Exclude<keyof typeof TONES, "text">;

/** The class that draws text in a role of the syntax palette. */
export function codeToneClass(tone: CodeTone): string {
  return TONES[tone].className;
}

const ITALIC = 1;
const BOLD = 2;

/** The classes for a token Shiki coloured with the Kaeso theme: its palette colour, italic and bold. Plain text gets an empty string. */
export function codeTokenClass(color: string | undefined, fontStyle: number | undefined): string {
  const style = fontStyle ?? 0;
  return [
    Object.values(TONES).find((t) => t.color === color?.toLowerCase())?.className ?? "",
    style & ITALIC ? "italic" : "",
    style & BOLD ? "font-semibold" : "",
  ]
    .filter(Boolean)
    .join(" ");
}
