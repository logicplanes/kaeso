import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Text: {
    description: "Text in the type scale.",
    props: {
      as: "The element or component to render as, such as `section` or `ul`.",
      children: "The text.",
      mono: "Geist Mono, for keys, ids, model ids and code.",
      tabular: "Tabular numerals. On by default for `metric`.",
      tone: "The text color role; status tones go next to words that say the same.",
      truncate: "One line with an ellipsis; the full text appears in a tooltip when it is cut.",
      variant: "The type style, from `micro` to `display`.",
      weight: "The weight, when it differs from the type style's.",
    },
  },
} satisfies ComponentDocs;
