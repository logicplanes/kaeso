import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Code: {
    description: "Inline code: Geist Mono 13 on fill-1.",
    props: {
      flow: "`text` for long code in running text, so it breaks with the sentence instead of jumping to its own line as one box; `cell` for the same in a table cell.",
      shape: "A pill on one line, rounded corners when it wraps, or `auto` to choose by length.",
    },
  },
} satisfies ComponentDocs;
