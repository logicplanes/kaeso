import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Link: {
    description: "A link. Router links come from `LinkProvider`.",
    props: {
      children: "The link's text.",
      external: "Opens in a new tab, adds rel and a 14px ↗. Detected from the href when not set.",
      href: "The address the link goes to.",
      variant: "Underlined in text (`inline`), quiet in lists (`quiet`) or on its own line with an arrow (`standalone`).",
    },
  },
} satisfies ComponentDocs;
