import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Timeline: {
    description:
      'Events down a 1px line (the audit log): each event marks the line with a short 3px bar in its tone, no dots; titles, and times on the right. A toned event\'s title starts with its tone in words for screen readers ("Critical: Key revoked"), and a time with `datetime` shows the exact date and time in a tooltip.',
    props: {
      events: "The events, oldest first.",
    },
  },
} satisfies ComponentDocs;
