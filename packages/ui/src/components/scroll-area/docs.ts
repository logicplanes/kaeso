import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ScrollArea: {
    description:
      "A scroll container with thin scrollbars that show on hover and darken while held, its content fading at every edge with more to scroll to; its focus ring sits above the fade.",
    props: {
      "aria-label": "Accessible name when the region is focusable.",
      children: "The content that scrolls.",
      orientation: "The directions it scrolls in.",
      viewportClassName: "Classes for the scrolling element, for layout only.",
    },
  },
} satisfies ComponentDocs;
