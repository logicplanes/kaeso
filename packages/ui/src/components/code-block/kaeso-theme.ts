const TONES = {
  text: { color: "var(--k-text)", className: "" },
  quiet: { color: "var(--k-text-tertiary)", className: "text-fg-tertiary" },
  secondary: { color: "var(--k-text-secondary)", className: "text-fg-secondary" },
  string: { color: "var(--k-info-text)", className: "text-info-text" },
  constant: { color: "var(--k-warning-text)", className: "text-warning-text" },
} as const;

const ITALIC = 1;
const BOLD = 2;

export function codeTokenClass(color: string | undefined, fontStyle: number | undefined): string {
  const style = fontStyle ?? 0;
  return [
    Object.values(TONES).find((tone) => tone.color === color?.toLowerCase())?.className ?? "",
    style & ITALIC ? "italic" : "",
    style & BOLD ? "font-semibold" : "",
  ]
    .filter(Boolean)
    .join(" ");
}

export const kaesoShikiTheme = {
  name: "kaeso",
  type: "light" as const,
  colors: { "editor.background": "transparent", "editor.foreground": TONES.text.color },
  settings: [
    { settings: { foreground: TONES.text.color, background: "transparent" } },
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: TONES.quiet.color, fontStyle: "italic" } },
    {
      scope: ["string", "string.quoted", "string.template", "punctuation.definition.string", "markup.inline.raw"],
      settings: { foreground: TONES.string.color },
    },
    {
      scope: ["constant.numeric", "constant.language.boolean", "constant.language.null", "constant.language"],
      settings: { foreground: TONES.constant.color },
    },
    {
      scope: ["keyword", "storage", "storage.type", "storage.modifier", "keyword.control", "keyword.operator.new", "variable.language"],
      settings: { foreground: TONES.text.color, fontStyle: "bold" },
    },
    {
      scope: ["punctuation", "meta.brace", "punctuation.separator", "punctuation.terminator", "keyword.operator", "meta.tag punctuation"],
      settings: { foreground: TONES.quiet.color },
    },
    { scope: ["entity.name.function", "support.function", "entity.name.tag", "support.class.component"], settings: { foreground: TONES.text.color } },
    {
      scope: ["support.type.property-name", "meta.object-literal.key", "entity.other.attribute-name", "variable.other.property"],
      settings: { foreground: TONES.text.color },
    },
    { scope: ["entity.name.type", "support.type", "entity.other.inherited-class"], settings: { foreground: TONES.secondary.color } },
  ],
};
