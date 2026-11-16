import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  TableOfContents: {
    description:
      '"On this page" beside the text: one row per heading between hairlines, the heading in view at full strength with the line above it in ink, the others faded. A subheading\'s row and its line start 12px in.',
    props: {
      active: "Controlled active id; otherwise it follows the scroll position.",
      items: "The headings to list, with their depth.",
      title: 'Defaults to "On this page" in the provider\'s language.',
    },
  },
} satisfies ComponentDocs;
