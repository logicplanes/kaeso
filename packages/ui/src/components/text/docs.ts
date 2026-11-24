import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Text: {
    description: "Text in the type scale.",
    props: {
      as: "The element or component to render as, such as `section` or `ul`.",
      children: "The text.",
      mono: "Geist Mono, for keys, ids, model ids and code.",
      tabular: "Tabular numerals. On by default for `metric`.",
      lines:
        "Below 1024px, wraps onto at most two lines and always keeps their room, so a line that arrives late, such as a chart's note, moves nothing under it; from 1024px it keeps one.",
      tone: "The text color role; status tones go next to words that say the same.",
      truncate: "One line with an ellipsis; when it is cut, plain text becomes a focus stop whose tooltip shows all of it.",
      variant: "The type style, from `micro` to `display`.",
      weight: "The weight, when it differs from the type style's.",
    },
  },
} satisfies ComponentDocs;
