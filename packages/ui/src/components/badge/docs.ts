import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Badge: {
    description: "A small pill label: 22px, 12/500. Words carry meaning; color supports them.",
    props: {
      children: "The badge's text, one or two words.",
      tone: "The status it marks; keep it next to words that say the same.",
    },
  },
} satisfies ComponentDocs;
