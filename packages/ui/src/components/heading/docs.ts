import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Heading: {
    description: "Heading levels map to the title tokens (4.3).",
    props: {
      balance: "Balances the lines of a heading that wraps.",
      level: "The heading level in the document, independent of its size.",
      size: "The type style, from `title-sm` to `display`.",
    },
  },
} satisfies ComponentDocs;
