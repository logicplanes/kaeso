import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Empty: {
    description: "An empty state: a title, one sentence, one or two actions. No icon, no illustration.",
    props: {
      actions: "One primary and at most one secondary action.",
      children: "One sentence that says what to do, and an action.",
      description: "One sentence: what is absent and the next step.",
      size: "How much room it takes: a whole page (`lg`), a section (`md`) or a row (`compact`).",
      title: "What is missing, in a few words.",
    },
  },
} satisfies ComponentDocs;
