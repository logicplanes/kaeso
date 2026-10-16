import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Status: {
    description: "The same component under its plain name.",
    props: {
      children: "The word that carries the meaning.",
      size: "The text size.",
      tone: "The status it marks.",
    },
  },
  StatusDot: {
    description: 'A status word: "Active", "Expiring", "Down".',
    props: {
      children: "The word that carries the meaning.",
      size: "The dot's size.",
      tone: "The status the dot marks; keep it next to words that say the same.",
    },
  },
} satisfies ComponentDocs;
