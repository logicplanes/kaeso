import type { ComponentDocs } from "../../component-docs";

export const docs = {
  ScrollArea: {
    description:
      "A scroll container with thin scrollbars in the strong line color that show on hover (5.1), its content fading at an edge with more to scroll to.",
    props: {
      "aria-label": "Accessible name when the region is focusable.",
      children: "The content that scrolls.",
      orientation: "The directions it scrolls in.",
      viewportClassName: "Classes for the scrolling element, for layout only.",
    },
  },
} satisfies ComponentDocs;
