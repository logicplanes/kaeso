import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  TableOfContents: {
    description: '"On this page": the heading in view in text color with a 2px ink bar on the rail.',
    props: {
      active: "Controlled active id; otherwise it follows the scroll position.",
      items: "The headings to list, with their depth.",
      title: 'Defaults to "On this page" in the provider\'s language.',
    },
  },
} satisfies ComponentDocs;
