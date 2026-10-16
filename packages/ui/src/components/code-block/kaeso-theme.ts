export const kaesoShikiTheme = {
  name: "kaeso",
  type: "light" as const,
  colors: { "editor.background": "transparent", "editor.foreground": "var(--k-text)" },
  settings: [
    { settings: { foreground: "var(--k-text)", background: "transparent" } },
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "var(--k-text-tertiary)", fontStyle: "italic" } },
    {
      scope: ["string", "string.quoted", "string.template", "punctuation.definition.string", "markup.inline.raw"],
      settings: { foreground: "var(--k-info-text)" },
    },
    {
      scope: ["constant.numeric", "constant.language.boolean", "constant.language.null", "constant.language"],
      settings: { foreground: "var(--k-warning-text)" },
    },
    {
      scope: ["keyword", "storage", "storage.type", "storage.modifier", "keyword.control", "keyword.operator.new", "variable.language"],
      settings: { foreground: "var(--k-text)", fontStyle: "bold" },
    },
    {
      scope: ["punctuation", "meta.brace", "punctuation.separator", "punctuation.terminator", "keyword.operator", "meta.tag punctuation"],
      settings: { foreground: "var(--k-text-tertiary)" },
    },
    { scope: ["entity.name.function", "support.function", "entity.name.tag", "support.class.component"], settings: { foreground: "var(--k-text)" } },
    {
      scope: ["support.type.property-name", "meta.object-literal.key", "entity.other.attribute-name", "variable.other.property"],
      settings: { foreground: "var(--k-text)" },
    },
    { scope: ["entity.name.type", "support.type", "entity.other.inherited-class"], settings: { foreground: "var(--k-text-secondary)" } },
  ],
};
