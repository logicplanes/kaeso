import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Sheet: {
    description: "A panel that slides in from the edge of the screen, for details and longer forms.",
    props: {
      actions: "Header actions before the close circle.",
      children: "The sheet's content.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      footer: "Content at the bottom, usually actions.",
      inline: "Render without a portal, for documentation and state matrices.",
      modal: "Keep the page behind interactive (non-modal).",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      side: "The edge it slides in from.",
      size: "The sheet's width, or its height from the bottom.",
      subtitle: "A mono subtitle for ids: `req_01J…`.",
      title: "The sheet's heading, also its accessible name.",
      trigger: "The element that opens it.",
    },
  },
} satisfies ComponentDocs;
