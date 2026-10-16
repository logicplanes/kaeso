import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  PillTabs: {
    description: 'Pill tabs for page headers ("All · Errors · Streamed").',
    props: {
      children: "The tab list and the panels.",
      defaultValue: "The selected tab on first render.",
      onValueChange: "Called with the selected tab.",
      value: "The selected tab, when you control it.",
      variant: "Pills, or underlined tabs.",
    },
  },
  Tabs: {
    description: "Underline tabs for detail pages.",
    props: {
      children: "The tab list and the panels.",
      defaultValue: "The selected tab on first render.",
      onValueChange: "Called with the selected tab.",
      value: "The selected tab, when you control it.",
      variant: "Underlined tabs for pages, or pills for small groups.",
    },
  },
} satisfies ComponentDocs;
