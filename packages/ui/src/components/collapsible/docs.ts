import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Accordion: {
    description: "Collapsible rows separated by hairlines, each the same pill row as Collapsible.",
    props: {
      defaultValue: "The sections open on first render.",
      items: "The sections, each with a title and its content.",
      multiple: "Several open at once.",
    },
  },
  Collapsible: {
    description:
      "A disclosure row: a pill with the title 12px in and a chevron at its end that turns in 200ms, the content on the title's line, a measured height animation.",
    props: {
      children: "The content that shows when it is open.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      hint: "Secondary text at the end of the row.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      title: "The heading that opens and closes it.",
    },
  },
} satisfies ComponentDocs;
