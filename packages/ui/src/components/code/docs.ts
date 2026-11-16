import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Code: {
    description: "Inline code: Geist Mono 13 on a fill-1 cut plate (radius 4).",
    props: {
      flow: "`text` for long code in running text, so it breaks with the sentence instead of jumping to its own line as one box; `cell` for the same in a table cell. The default box wraps inside itself when it is wider than its line.",
      shape:
        "`auto` (default) and `rounded` keep radius 4, so code in a sentence never mixes shapes; pass `pill` for one word standing alone, such as a model name in a table cell.",
    },
  },
} satisfies ComponentDocs;
