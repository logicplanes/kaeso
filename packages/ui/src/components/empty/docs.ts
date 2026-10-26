import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Empty: {
    description:
      "An empty state: a title, one sentence, one action. No icon, no illustration. The `lg` title is a heading (h2) for a page that is empty; `md` and `compact` titles are text, so an empty table, list or section adds no heading to the page's outline.",
    props: {
      actions: "The one action that makes something appear; put a link to read more in the description.",
      children: "One sentence that says what to do, and an action.",
      description: "One sentence: what is absent and the next step.",
      size: "How much room it takes: a whole page (`lg`, with a heading), a section (`md`) or a row (`compact`).",
      title: "What is missing, in a few words.",
    },
  },
} satisfies ComponentDocs;
