import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Tabs: {
    description:
      'Tabs: underlined for sections of a detail page, or pills (`variant="pill"`) for the views of a page under its header ("All · Errors · Streamed").',
    props: {
      children: "The tab list and the panels.",
      defaultValue: "The selected tab on first render.",
      onValueChange: "Called with the selected tab.",
      value: "The selected tab, when you control it.",
      variant: "`underline` for the sections of a detail page; `pill` for a page's views, right under its header.",
    },
  },
} satisfies ComponentDocs;
