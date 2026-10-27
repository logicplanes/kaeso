import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Sheet: {
    description:
      "A panel that slides in from the edge of the screen, for details and longer forms. While it slides out it keeps showing what it showed when it was open, so a page can let go of the row as soon as it closes.",
    props: {
      actions: "Header actions before the close circle.",
      children: "The sheet's content.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      footer: "Content at the bottom, usually actions.",
      inline: "Render without a portal, for documentation and state matrices.",
      modal:
        "`false` keeps the page behind interactive, for an inspector: a press on the page leaves the sheet open, so the next row can fill it; Escape and the close circle close it.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      side: "The edge it slides in from.",
      size: "The sheet's width from the side; a sheet from the bottom takes its content's height and ignores it.",
      subtitle: "A mono subtitle for ids, shown whole: `req_01J…`.",
      title: "The sheet's heading, also its accessible name; a long one wraps.",
      trigger: "The element that opens it.",
    },
  },
} satisfies ComponentDocs;
