import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Empty: {
    description:
      "An empty state: a strong title, one sentence, one action. No icon, no illustration. On a page (`lg`) and in a section (`md`) it hangs from the edge the page's title and rows start on, its title set large and tight; in a table or list row (`compact`) it is one short statement, centered. The title is a heading (h2) at every size.",
    props: {
      actions: "The one action that makes something appear; put a link to read more in the description.",
      children: "One sentence that says what to do, and an action.",
      description: "One sentence: what is absent and the next step.",
      size: "How much room it takes: a whole page (`lg`, with a heading), a section (`md`) or a row (`compact`).",
      title: "What is missing, in a few words.",
    },
  },
} satisfies ComponentDocs;
