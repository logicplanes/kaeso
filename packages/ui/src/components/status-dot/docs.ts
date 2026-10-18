import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Status: {
    description: "The same component under its plain name.",
    props: {
      children: "The word that carries the meaning.",
      icon: "A mark before the word in the same color, e.g. `icons.success` for “Up to date”.",
      size: "The text size.",
      tone: "The status it marks.",
    },
  },
  StatusDot: {
    description: 'A status word: "Active", "Expiring", "Down".',
    props: {
      children: "The word that carries the meaning.",
      icon: "A mark before the word in the same color, e.g. `icons.success` for “Up to date”.",
      size: "The dot's size.",
      tone: "The status the dot marks; keep it next to words that say the same.",
    },
  },
} satisfies ComponentDocs;
