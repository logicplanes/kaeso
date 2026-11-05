import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Status: {
    description: 'A status word in its tone\'s color, with an optional icon: "Active", "Expiring", "Down". Never color alone.',
    props: {
      children: "The word that carries the meaning.",
      icon: "A mark before the word in the same color, e.g. `icons.success` for “Up to date”.",
      size: "The text size: 14px, or 13px (`sm`) with a 14px icon.",
      tone: "The status the word marks; keep it next to words that say the same.",
    },
  },
} satisfies ComponentDocs;
