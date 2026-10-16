import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Separator: {
    description: "A 1px line. Use space and tone first; a line only where they cannot separate.",
    props: {
      label: 'A caption in the middle of a horizontal line ("or").',
      orientation: "A horizontal or vertical line.",
      tone: "A subtle or a clearer line.",
    },
  },
} satisfies ComponentDocs;
