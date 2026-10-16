import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Timeline: {
    description: "Events down a 1px line (the audit log): each event marks the line with a short 3px bar in its tone, no dots; titles, and times on the right.",
    props: {
      events: "The events, oldest first.",
    },
  },
} satisfies ComponentDocs;
