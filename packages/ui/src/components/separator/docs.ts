import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Separator: {
    description: "A 1px line. Use space and tone first; a line only where they cannot separate.",
    props: {
      label: 'A caption in the middle of a horizontal line ("or").',
      orientation: "A horizontal or vertical line.",
      tone: "A subtle line inside groups, a clearer one between regions, `strong` for a section head's rule, `marker` for a list item's short hairline, or `ink` in the text colour for a quotation's rule (2px wide when vertical).",
    },
  },
} satisfies ComponentDocs;
